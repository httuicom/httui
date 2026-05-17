import { afterEach, describe, expect, it } from "vitest";

import {
  getFileMtime,
  getFileSettings,
  setFileAutoCapture,
  setFileDocheaderCompact,
} from "@/lib/tauri/files";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";

afterEach(() => {
  clearTauriMocks();
});

describe("files Tauri wrappers", () => {
  it("getFileMtime invokes 'get_file_mtime' and returns the mocked value", async () => {
    mockTauriCommand("get_file_mtime", (args) => {
      expect(args).toEqual({ vaultPath: "/v", filePath: "a.md" });
      return 1700000000000;
    });
    expect(await getFileMtime("/v", "a.md")).toBe(1700000000000);
  });

  it("getFileMtime propagates null", async () => {
    mockTauriCommand("get_file_mtime", () => null);
    expect(await getFileMtime("/v", "missing.md")).toBeNull();
  });

  it("getFileSettings invokes 'get_file_settings' and returns the FileSettings shape", async () => {
    mockTauriCommand("get_file_settings", (args) => {
      expect(args).toEqual({ vaultPath: "/v", filePath: "a.md" });
      return { auto_capture: true };
    });
    const out = await getFileSettings("/v", "a.md");
    expect(out.auto_capture).toBe(true);
  });

  it("setFileAutoCapture invokes 'set_file_auto_capture' with the right args", async () => {
    let captured: unknown = null;
    mockTauriCommand("set_file_auto_capture", (args) => {
      captured = args;
      return null;
    });
    await setFileAutoCapture("/v", "a.md", true);
    expect(captured).toEqual({
      vaultPath: "/v",
      filePath: "a.md",
      autoCapture: true,
    });
  });

  it("setFileDocheaderCompact invokes 'set_file_docheader_compact' with the right args", async () => {
    let captured: unknown = null;
    mockTauriCommand("set_file_docheader_compact", (args) => {
      captured = args;
      return null;
    });
    await setFileDocheaderCompact("/v", "a.md", true);
    expect(captured).toEqual({
      vaultPath: "/v",
      filePath: "a.md",
      compact: true,
    });
  });

  it("setFileDocheaderCompact propagates a rejected invoke", async () => {
    mockTauriCommand("set_file_docheader_compact", () => {
      throw new Error("vault not writable");
    });
    await expect(setFileDocheaderCompact("/v", "a.md", true)).rejects.toThrow(
      /vault not writable/,
    );
  });
});
