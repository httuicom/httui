//! httui-lsp sidecar: lazy spawn plus a Content-Length framing bridge
//! between the webview (plain JSON strings over Tauri IPC) and the
//! language server's stdio. The webview cannot speak stdio, so messages
//! cross as `lsp_send` invokes one way and `lsp:message` events the
//! other way.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Runtime};

struct Running {
    child: Child,
    stdin: ChildStdin,
}

static STATE: Mutex<Option<Running>> = Mutex::new(None);

/// `HTTUI_LSP_BIN` overrides for development; otherwise the launcher
/// convention applies (binary on PATH, shipped next to the app).
fn resolve_binary() -> String {
    match std::env::var("HTTUI_LSP_BIN") {
        Ok(p) if !p.is_empty() => p,
        _ => "httui-lsp".to_string(),
    }
}

/// Wrap a JSON message in LSP Content-Length framing.
fn frame(message: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", message.len(), message)
}

/// Read one framed message; `None` on EOF, broken pipe or malformed
/// framing (all of which mean the server connection is over).
fn read_framed(reader: &mut impl BufRead) -> Option<String> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            len = rest.trim().parse().ok();
        }
    }
    let len = len?;
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

fn read_loop<R: Runtime>(app: AppHandle<R>, stdout: ChildStdout) {
    let mut reader = BufReader::new(stdout);
    while let Some(message) = read_framed(&mut reader) {
        let _ = app.emit("lsp:message", message);
    }
    let _ = app.emit("lsp:exit", ());
}

#[tauri::command]
pub fn lsp_start<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let mut guard = STATE.lock().unwrap();
    if let Some(running) = guard.as_mut() {
        match running.child.try_wait() {
            Ok(None) => return Ok(()),
            _ => *guard = None,
        }
    }
    let bin = resolve_binary();
    let db = httui_core::paths::default_data_dir()
        .map_err(|e| e.to_string())?
        .join("notes.db");
    let mut child = Command::new(&bin)
        .arg("--db")
        .arg(&db)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn {bin}: {e}"))?;
    let stdin = child.stdin.take().ok_or("language server has no stdin")?;
    let stdout = child.stdout.take().ok_or("language server has no stdout")?;
    std::thread::spawn(move || read_loop(app, stdout));
    *guard = Some(Running { child, stdin });
    Ok(())
}

#[tauri::command]
pub fn lsp_send(message: String) -> Result<(), String> {
    let mut guard = STATE.lock().unwrap();
    let running = guard
        .as_mut()
        .ok_or("language server not running — call lsp_start first")?;
    running
        .stdin
        .write_all(frame(&message).as_bytes())
        .map_err(|e| e.to_string())?;
    running.stdin.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::mpsc;
    use tauri::Listener;

    #[test]
    fn frame_wraps_message_with_content_length() {
        assert_eq!(frame("{}"), "Content-Length: 2\r\n\r\n{}");
    }

    #[test]
    fn read_framed_parses_a_message() {
        let mut input = Cursor::new(b"Content-Length: 4\r\n\r\nabcd".to_vec());
        assert_eq!(read_framed(&mut input).as_deref(), Some("abcd"));
        assert_eq!(read_framed(&mut input), None);
    }

    #[test]
    fn read_framed_rejects_missing_header() {
        let mut input = Cursor::new(b"X-Other: 1\r\n\r\nabcd".to_vec());
        assert_eq!(read_framed(&mut input), None);
    }

    #[test]
    fn read_framed_handles_eof_mid_body() {
        let mut input = Cursor::new(b"Content-Length: 10\r\n\r\nab".to_vec());
        assert_eq!(read_framed(&mut input), None);
    }

    // env-var handling is asserted inside the round-trip test below:
    // cargo runs tests in parallel and HTTUI_LSP_BIN is process-global,
    // so only one test may touch it
    #[test]
    fn sidecar_round_trip_with_fake_server() {
        std::env::remove_var("HTTUI_LSP_BIN");
        assert_eq!(resolve_binary(), "httui-lsp");

        // a fake server that echoes framed stdin back to stdout
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-lsp.sh");
        std::fs::write(&script, "#!/bin/sh\nexec cat\n").unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        std::env::set_var("HTTUI_LSP_BIN", script.to_str().unwrap());
        assert_eq!(resolve_binary(), script.to_str().unwrap());

        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let (tx, rx) = mpsc::channel::<String>();
        handle.listen("lsp:message", move |event| {
            let _ = tx.send(event.payload().to_string());
        });

        assert!(lsp_send("early".into()).is_err(), "send before start fails");
        lsp_start(handle.clone()).expect("spawns fake server");
        lsp_start(handle.clone()).expect("second start is a no-op");
        lsp_send("{\"jsonrpc\":\"2.0\"}".into()).expect("send works");

        let echoed = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("message echoed back through the bridge");
        assert!(echoed.contains("jsonrpc"));

        std::env::remove_var("HTTUI_LSP_BIN");
        let mut guard = STATE.lock().unwrap();
        if let Some(mut running) = guard.take() {
            let _ = running.child.kill();
        }
    }
}
