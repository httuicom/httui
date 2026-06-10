// notifyBlockRan must never spin the server up by itself: with no
// editor-created client there are no open documents, so a refresh has
// nothing to republish.
import { afterEach, describe, expect, it, vi } from "vitest";

const notification = vi.fn();
vi.mock("@codemirror/lsp-client", () => ({
  LSPClient: class {
    notification = notification;
    connect() {
      return this;
    }
  },
  serverDiagnostics: () => [],
}));
vi.mock("../transport", () => ({
  createTauriLspTransport: () => ({}),
}));

import {
  fileUri,
  getLspClient,
  notifyBlockRan,
  resetLspClient,
} from "../client";

afterEach(() => {
  resetLspClient();
  notification.mockClear();
});

describe("notifyBlockRan", () => {
  it("is a no-op before any editor created the client", () => {
    notifyBlockRan();
    expect(notification).not.toHaveBeenCalled();
  });

  it("sends httui/refresh with object params once a client exists", () => {
    getLspClient();
    notifyBlockRan();
    expect(notification).toHaveBeenCalledWith("httui/refresh", {});
  });
});

describe("fileUri", () => {
  it("joins vault and file into an absolute file uri", () => {
    expect(fileUri("/v/ault", "notes/a.md")).toBe("file:///v/ault/notes/a.md");
  });
});
