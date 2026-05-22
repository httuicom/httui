use crossterm::event::{Event as CtEvent, KeyEvent, KeyEventKind};
use futures::{Stream, StreamExt};
use httui_core::db::schema_cache::SchemaEntry;
use httui_core::executor::db::types::DbResponse;
use httui_core::executor::http::types::HttpResponse;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::error::TuiResult;

/// All events the main loop reacts to.
///
/// Future variants for streaming block execution and file watcher events
/// land here without changing the dispatcher shape — / 22.
#[derive(Debug)]
#[allow(dead_code)] // Resize / Quit are wired but not yet consumed by the scaffold.
pub enum AppEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
    Quit,
    /// A backgrounded DB query (kicked off by `apply_run_block` /
    /// `load_more_db_block`) finished. The main loop folds the
    /// outcome back into the block at `segment_idx` and clears the
    /// app's `running_query` slot. `kind` distinguishes a fresh
    /// run from a paginated load-more so the merge logic picks the
    /// right strategy.
    DbBlockResult {
        segment_idx: usize,
        kind: DbBlockResultKind,
        outcome: Result<DbResponse, String>,
    },
    /// A backgrounded HTTP request finished. The main loop folds
    /// the response into `block.cached_result` and clears the
    /// `running_query` slot.
    HttpBlockResult {
        segment_idx: usize,
        outcome: Result<HttpResponse, String>,
    },
    /// A backgrounded schema introspection finished. Spawned by
    /// `App::ensure_schema_loaded` (typically right after the user
    /// confirms a connection in the picker). The main loop folds the
    /// result into `App.schema_cache` and clears the pending flag so
    /// retries are possible after an error.
    SchemaLoaded {
        connection_id: String,
        result: Result<Vec<SchemaEntry>, String>,
    },
    /// The async FTS5 index rebuild kicked off by `<C-f>` finished.
    /// Main loop flips `App.content_search_index_built = true` and
    /// runs the current query — the user may have typed during the
    /// build, so we re-query against the freshly populated index.
    /// Failure surfaces as a status error and clears the modal.
    ContentSearchIndexBuilt {
        result: Result<(), String>,
    },
    /// The filesystem watcher saw a modify/create event on the
    /// active document's path. The main loop compares disk content
    /// to the buffer; if equal, the event was our own write and we
    /// silently drop it. If different + clean, the buffer is
    /// reloaded silently. If different + dirty, a status warning
    /// surfaces (the user has unsaved changes that would be lost).
    FileChangedExternally {
        path: PathBuf,
    },
}

/// Why this DB result was produced — used by the main loop to pick
/// between "replace the cached_result" (fresh run), "append the
/// new page's rows" (load-more), and "merge into the plan slot
/// without touching results" (EXPLAIN).
#[derive(Debug)]
pub enum DbBlockResultKind {
    Run,
    LoadMore,
    /// `<C-x>` (EXPLAIN) — the query was wrapped in the dialect's
    /// EXPLAIN keyword. Result lands in `cached_result["plan"]`,
    /// not `cached_result.results`, so the original query's output
    /// isn't destroyed by inspecting its plan.
    Explain,
}

/// Spawns a background task that drains `crossterm`'s event stream and
/// emits a periodic `Tick` for animation/polling. The receiver is owned
/// by the main loop.
pub struct EventLoop {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    tx: mpsc::UnboundedSender<AppEvent>,
    _handle: tokio::task::JoinHandle<()>,
}

impl EventLoop {
    pub fn start(tick_rate: Duration) -> TuiResult<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        // `EventStream::new()` stays inside the async block so it is
        // constructed on the spawned task, not the caller: it binds to
        // the process TTY and panics when there is none (headless
        // tests). The loop body itself lives in `run_event_loop`,
        // generic over the stream so tests inject a fake source.
        let handle = tokio::spawn(async move {
            run_event_loop(crossterm::event::EventStream::new(), tx_clone, tick_rate).await;
        });
        Ok(Self {
            rx,
            tx,
            _handle: handle,
        })
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    /// Hand a cloneable sender to dispatch handlers that need to
    /// inject events from spawned tasks (currently the async DB
    /// executor). The receiver continues to flow through `next()`.
    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.tx.clone()
    }
}

/// Drive the terminal event loop: forward each event from `stream`
/// (filtered through [`map_crossterm_event`]) plus a periodic `Tick`
/// into `tx`. Exits when `tx`'s receiver is dropped.
///
/// Generic over the stream so tests inject a fake source — the real
/// `crossterm::event::EventStream` binds to the TTY and cannot run
/// headless.
async fn run_event_loop<S>(mut stream: S, tx: mpsc::UnboundedSender<AppEvent>, tick_rate: Duration)
where
    S: Stream<Item = io::Result<CtEvent>> + Unpin,
{
    let mut tick = tokio::time::interval(tick_rate);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = tick.tick() => {
                if tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
            Some(Ok(ev)) = stream.next() => {
                let Some(mapped) = map_crossterm_event(ev) else {
                    continue;
                };
                if tx.send(mapped).is_err() {
                    break;
                }
            }
        }
    }
}

/// Translate a `crossterm` event into an [`AppEvent`], or `None` for
/// events the dispatcher doesn't consume.
///
/// Key *release* events are dropped: with the kitty keyboard protocol
/// (enabled in `terminal::setup`) the terminal emits Press/Repeat AND
/// Release for a single physical keypress. The app's keymaps assume
/// one event per press, so without this filter every keystroke would
/// fire twice. `Repeat` is kept — held-key autorepeat is a real press.
fn map_crossterm_event(ev: CtEvent) -> Option<AppEvent> {
    match ev {
        CtEvent::Key(k) if k.kind == KeyEventKind::Release => None,
        CtEvent::Key(k) => Some(AppEvent::Key(k)),
        CtEvent::Resize(c, r) => Some(AppEvent::Resize(c, r)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind};

    #[test]
    fn maps_key_press_to_app_event() {
        let ev = CtEvent::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(matches!(map_crossterm_event(ev), Some(AppEvent::Key(_))));
    }

    #[test]
    fn drops_key_release_events() {
        // Kitty protocol emits a Release for every key; processing it
        // would make every keystroke fire twice.
        let mut k = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        k.kind = KeyEventKind::Release;
        assert!(map_crossterm_event(CtEvent::Key(k)).is_none());
    }

    #[test]
    fn keeps_key_repeat_events() {
        let mut k = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        k.kind = KeyEventKind::Repeat;
        assert!(matches!(
            map_crossterm_event(CtEvent::Key(k)),
            Some(AppEvent::Key(_))
        ));
    }

    #[test]
    fn maps_resize_event() {
        assert!(matches!(
            map_crossterm_event(CtEvent::Resize(80, 24)),
            Some(AppEvent::Resize(80, 24))
        ));
    }

    #[test]
    fn drops_mouse_events() {
        let ev = CtEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert!(map_crossterm_event(ev).is_none());
    }

    #[tokio::test]
    async fn sender_injects_events_into_the_loop() {
        // `next()` reads straight off the mpsc receiver, so an event
        // pushed through `sender()` is observable without any TTY
        // input. A 1h tick rate keeps the tick branch from racing it.
        let mut el = EventLoop::start(Duration::from_secs(3600)).unwrap();
        el.sender().send(AppEvent::Quit).unwrap();
        let ev = tokio::time::timeout(Duration::from_secs(2), el.next())
            .await
            .expect("timed out waiting for the injected event");
        assert!(matches!(ev, Some(AppEvent::Quit)));
    }

    #[tokio::test]
    async fn run_event_loop_forwards_and_filters_stream_events() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let press = CtEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        let mut release_key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        release_key.kind = KeyEventKind::Release;
        // Press, release (must be filtered), resize — then EOF.
        let fake = futures::stream::iter(vec![
            Ok(press),
            Ok(CtEvent::Key(release_key)),
            Ok(CtEvent::Resize(120, 40)),
        ]);
        // A 1h tick rate: `interval`'s first tick fires immediately
        // (the lone Tick), then the stream events drain. Once the
        // stream hits EOF the loop parks on the 1h tick, so the
        // collecting `timeout` below ends the test deterministically.
        tokio::spawn(run_event_loop(fake, tx, Duration::from_secs(3600)));

        let mut events = Vec::new();
        while let Ok(Some(ev)) =
            tokio::time::timeout(Duration::from_millis(200), rx.recv()).await
        {
            events.push(ev);
        }
        let keys = events.iter().filter(|e| matches!(e, AppEvent::Key(_))).count();
        let resizes = events
            .iter()
            .filter(|e| matches!(e, AppEvent::Resize(..)))
            .count();
        let ticks = events.iter().filter(|e| matches!(e, AppEvent::Tick)).count();
        assert_eq!(keys, 1, "press forwarded, release filtered out");
        assert_eq!(resizes, 1);
        assert_eq!(ticks, 1, "only the immediate first tick");
    }

    #[tokio::test]
    async fn run_event_loop_exits_when_receiver_is_dropped() {
        let (tx, rx) = mpsc::unbounded_channel();
        // Empty stream + a fast tick: the first tick's send fails once
        // the receiver is gone, breaking the loop.
        let fake = futures::stream::iter(Vec::<io::Result<CtEvent>>::new());
        let handle = tokio::spawn(run_event_loop(fake, tx, Duration::from_millis(2)));
        drop(rx);
        let joined = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(joined.is_ok(), "loop must exit once the receiver is dropped");
    }
}
