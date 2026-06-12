import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

import { useHttpCodegenSnippets } from "../useHttpCodegenSnippets";
import type { HttpMessageParsed } from "@/lib/blocks/http-message";

// ── Mocks (every external dep) ─────────────────────────────────

const collectBlocksAboveCMMock = vi.fn();
vi.mock("@/lib/blocks/document", () => ({
  collectBlocksAboveCM: (...args: unknown[]) =>
    collectBlocksAboveCMMock(...args),
}));

const resolveAllReferencesMock = vi.fn();
vi.mock("@/lib/blocks/references", () => ({
  resolveAllReferences: (...args: unknown[]) =>
    resolveAllReferencesMock(...args),
}));

const getActiveVariablesMock = vi.fn();
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({
      getActiveVariables: () => getActiveVariablesMock(),
    }),
  },
}));

vi.mock("@/lib/blocks/http-codegen", () => ({
  toCurl: (r: unknown) => `CURL:${JSON.stringify(r)}`,
  toFetch: () => "FETCH",
  toPython: () => "PYTHON",
  toHTTPie: () => "HTTPIE",
  toHttpFile: () => "HTTPFILE",
}));

const saveDialogMock = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: (...args: unknown[]) => saveDialogMock(...args),
}));

const writeFileMock = vi.fn();
vi.mock("@tauri-apps/plugin-fs", () => ({
  writeFile: (...args: unknown[]) => writeFileMock(...args),
}));

// ── Fixtures + helpers ─────────────────────────────────────────

const fakeView = { state: { doc: { id: "doc-1" } } } as unknown as Parameters<
  typeof useHttpCodegenSnippets
>[0]["view"];

const baseParsed = (
  overrides: Partial<HttpMessageParsed> = {},
): HttpMessageParsed => ({
  method: "GET",
  url: "https://api.example.com/users",
  params: [],
  headers: [],
  body: "",
  ...overrides,
});

// ── Tests ──────────────────────────────────────────────────────

describe("useHttpCodegenSnippets", () => {
  beforeEach(() => {
    collectBlocksAboveCMMock.mockReset();
    resolveAllReferencesMock.mockReset();
    getActiveVariablesMock.mockReset();
    saveDialogMock.mockReset();
    writeFileMock.mockReset();
  });

  it("starts with snippets=null before the async load settles", () => {
    collectBlocksAboveCMMock.mockReturnValue(new Promise(() => {}));
    getActiveVariablesMock.mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    expect(result.current.snippets).toBeNull();
  });

  it("populates snippets with one entry per format after the load", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());
    expect(Object.keys(result.current.snippets!)).toEqual(
      expect.arrayContaining([
        "curl",
        "fetch",
        "python",
        "httpie",
        "http-file",
      ]),
    );
    expect(result.current.snippets!.fetch).toBe("FETCH");
    expect(result.current.snippets!.python).toBe("PYTHON");
  });

  it("resolves URL / params / headers / body via resolveAllReferences before codegen", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({ TOK: "abc" });
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text.replace("{{TOK}}", "abc"),
    }));
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed({
          url: "https://api.example.com/{{TOK}}",
          body: "{{TOK}}",
          headers: [{ key: "X", value: "{{TOK}}", enabled: true }],
        }),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());
    const curlOut = result.current.snippets!.curl;
    expect(curlOut).toContain("abc");
    expect(curlOut).not.toContain("{{TOK}}");
  });

  it("sets snippets to null when the async pipeline throws", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockRejectedValue(new Error("env boom"));
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    // Allow the rejection to settle.
    await new Promise((r) => setTimeout(r, 5));
    expect(result.current.snippets).toBeNull();
  });

  it("handleSendAs is a no-op when snippets are not loaded yet", async () => {
    collectBlocksAboveCMMock.mockReturnValue(new Promise(() => {}));
    getActiveVariablesMock.mockReturnValue(new Promise(() => {}));
    const writeText = vi.fn();
    Object.assign(navigator, { clipboard: { writeText } });
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    act(() => result.current.handleSendAs("curl"));
    expect(writeText).not.toHaveBeenCalled();
  });

  it("handleSendAs writes the formatted snippet to the clipboard (curl path)", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());

    act(() => result.current.handleSendAs("fetch"));
    expect(writeText).toHaveBeenCalledWith("FETCH");
  });

  it("handleSendAs swallows clipboard rejection", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    const writeText = vi.fn().mockRejectedValue(new Error("clipboard blocked"));
    Object.assign(navigator, { clipboard: { writeText } });
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());
    // No throw / no unhandled rejection.
    act(() => result.current.handleSendAs("python"));
    await new Promise((r) => setTimeout(r, 5));
    expect(writeText).toHaveBeenCalledWith("PYTHON");
  });

  it("handleSendAs('http-file') opens save dialog + writes file", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    saveDialogMock.mockResolvedValue("/tmp/picked.http");
    writeFileMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());

    act(() => result.current.handleSendAs("http-file"));
    await waitFor(() => expect(writeFileMock).toHaveBeenCalled());
    expect(saveDialogMock).toHaveBeenCalledWith({
      defaultPath: "req1.http",
      filters: [{ name: "HTTP request", extensions: ["http", "rest"] }],
    });
    expect(writeFileMock.mock.calls[0][0]).toBe("/tmp/picked.http");
    // jsdom/vitest cross-context: `Uint8Array` may not be reference-identical
    // — assert by constructor name instead of `toBeInstanceOf`.
    expect(writeFileMock.mock.calls[0][1].constructor.name).toBe("Uint8Array");
  });

  it("falls back to 'request.http' when no alias is set", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    saveDialogMock.mockResolvedValue("/tmp/x.http");
    writeFileMock.mockResolvedValue(undefined);
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: undefined,
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());

    act(() => result.current.handleSendAs("http-file"));
    await waitFor(() => expect(saveDialogMock).toHaveBeenCalled());
    expect(saveDialogMock.mock.calls[0][0].defaultPath).toBe("request.http");
  });

  it("save dialog cancel (null path) skips writeFile", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    saveDialogMock.mockResolvedValue(null);
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());

    act(() => result.current.handleSendAs("http-file"));
    await new Promise((r) => setTimeout(r, 5));
    expect(writeFileMock).not.toHaveBeenCalled();
  });

  it("save dialog error surfaces via window.alert", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    saveDialogMock.mockRejectedValue(new Error("perm denied"));
    const alertSpy = vi.spyOn(window, "alert").mockImplementation(() => {});
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());

    act(() => result.current.handleSendAs("http-file"));
    await waitFor(() => expect(alertSpy).toHaveBeenCalled());
    expect(alertSpy.mock.calls[0][0]).toContain("perm denied");
    alertSpy.mockRestore();
  });

  it("copyAsCurl is a thin shortcut to handleSendAs('curl')", async () => {
    collectBlocksAboveCMMock.mockResolvedValue([]);
    getActiveVariablesMock.mockResolvedValue({});
    resolveAllReferencesMock.mockImplementation((text: string) => ({
      resolved: text,
    }));
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    const { result } = renderHook(() =>
      useHttpCodegenSnippets({
        view: fakeView,
        blockFrom: 0,
        filePath: "x.md",
        parsed: baseParsed(),
        alias: "req1",
      }),
    );
    await waitFor(() => expect(result.current.snippets).not.toBeNull());

    act(() => result.current.copyAsCurl());
    // Mocked toCurl returns `CURL:{json}` — confirm it landed in clipboard.
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("CURL:"));
  });
});
