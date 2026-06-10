import { describe, it, expect, afterEach } from "vitest";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { emitTauriEvent, clearTauriListeners } from "@/test/mocks/tauri-event";
import { createTauriLspTransport } from "../transport";
import { fileUri } from "../client";

const flush = () => new Promise((r) => setTimeout(r, 0));

afterEach(() => {
  clearTauriMocks();
  clearTauriListeners();
});

describe("createTauriLspTransport", () => {
  it("queues messages until the sidecar starts, then flushes in order", async () => {
    const sent: string[] = [];
    let started = 0;
    mockTauriCommand("lsp_start", () => {
      started += 1;
    });
    mockTauriCommand("lsp_send", (args) => {
      sent.push((args as { message: string }).message);
    });

    const transport = createTauriLspTransport();
    transport.send("one");
    transport.send("two");
    expect(sent).toEqual([]);

    await flush();
    expect(started).toBe(1);
    expect(sent).toEqual(["one", "two"]);

    transport.send("three");
    await flush();
    expect(sent).toEqual(["one", "two", "three"]);
  });

  it("starts the sidecar only once across many sends", async () => {
    let started = 0;
    mockTauriCommand("lsp_start", () => {
      started += 1;
    });
    mockTauriCommand("lsp_send", () => {});

    const transport = createTauriLspTransport();
    transport.send("a");
    transport.send("b");
    transport.send("c");
    await flush();
    expect(started).toBe(1);
  });

  it("delivers lsp:message events to subscribers", async () => {
    mockTauriCommand("lsp_start", () => {});
    const transport = createTauriLspTransport();
    await flush(); // let the listen() registrations land

    const received: string[] = [];
    const handler = (value: string) => received.push(value);
    transport.subscribe(handler);
    emitTauriEvent("lsp:message", '{"jsonrpc":"2.0"}');
    expect(received).toEqual(['{"jsonrpc":"2.0"}']);

    transport.unsubscribe(handler);
    emitTauriEvent("lsp:message", "ignored");
    expect(received).toEqual(['{"jsonrpc":"2.0"}']);
  });

  it("re-starts the sidecar after lsp:exit", async () => {
    let started = 0;
    const sent: string[] = [];
    mockTauriCommand("lsp_start", () => {
      started += 1;
    });
    mockTauriCommand("lsp_send", (args) => {
      sent.push((args as { message: string }).message);
    });

    const transport = createTauriLspTransport();
    transport.send("before");
    await flush();
    expect(started).toBe(1);

    emitTauriEvent("lsp:exit", null);
    transport.send("after");
    await flush();
    expect(started).toBe(2);
    expect(sent).toEqual(["before", "after"]);
  });
});

describe("fileUri", () => {
  it("joins vault and file into a file:// uri", () => {
    expect(fileUri("/Users/x/vault", "notes/api.md")).toBe(
      "file:///Users/x/vault/notes/api.md",
    );
  });

  it("collapses duplicate slashes and encodes spaces", () => {
    expect(fileUri("/Users/x/vault/", "/minha nota.md")).toBe(
      "file:///Users/x/vault/minha%20nota.md",
    );
  });
});
