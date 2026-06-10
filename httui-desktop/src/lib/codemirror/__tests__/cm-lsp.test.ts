import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mockTauriCommand, clearTauriMocks } from "@/test/mocks/tauri";
import { clearTauriListeners } from "@/test/mocks/tauri-event";
import { useWorkspaceStore } from "@/stores/workspace";
import { resetLspClient } from "@/lib/lsp/client";
import { createLspExtension } from "../cm-lsp";

beforeEach(() => {
  resetLspClient();
  mockTauriCommand("lsp_start", () => {});
  mockTauriCommand("lsp_send", () => {});
});

afterEach(() => {
  clearTauriMocks();
  clearTauriListeners();
  useWorkspaceStore.setState({ vaultPath: null });
});

describe("createLspExtension", () => {
  it("is empty without an active vault", () => {
    useWorkspaceStore.setState({ vaultPath: null });
    expect(createLspExtension("notes/a.md")).toEqual([]);
  });

  it("produces the plugin, completion and hover extensions for a vault file", () => {
    useWorkspaceStore.setState({ vaultPath: "/tmp/vault" });
    const exts = createLspExtension("notes/a.md");
    expect(exts.length).toBe(3);
    for (const ext of exts) {
      expect(ext).toBeTruthy();
    }
  });
});
