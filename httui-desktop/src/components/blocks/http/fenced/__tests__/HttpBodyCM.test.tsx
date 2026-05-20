import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderWithProviders } from "@/test/render";

// Keep CodeMirror out of jsdom (project convention) and record the props
// each render hands it, so we can assert the extension array is stable
// across `value` changes — the regression this test guards.
const recordedExtensions: unknown[] = [];
const fakeView = { dispatch: vi.fn() };
let created = false;

vi.mock("@uiw/react-codemirror", () => ({
  default: (props: {
    extensions?: unknown;
    onCreateEditor?: (v: unknown) => void;
  }) => {
    recordedExtensions.push(props.extensions);
    if (!created) {
      created = true;
      props.onCreateEditor?.(fakeView);
    }
    return null;
  },
}));

// Imported after the mock is registered.
import {
  HttpBodyCM,
  looksLikeJsonBody,
} from "@/components/blocks/http/fenced/HttpFencedPanel";

beforeEach(() => {
  recordedExtensions.length = 0;
  fakeView.dispatch.mockClear();
  created = false;
});
afterEach(() => vi.clearAllMocks());

describe("looksLikeJsonBody", () => {
  it("is true for objects/arrays, ignoring leading whitespace", () => {
    expect(looksLikeJsonBody('{"a":1}')).toBe(true);
    expect(looksLikeJsonBody("  \n [1,2]")).toBe(true);
  });
  it("is false for plain text and empty bodies", () => {
    expect(looksLikeJsonBody("hello world")).toBe(false);
    expect(looksLikeJsonBody("")).toBe(false);
    expect(looksLikeJsonBody("   ")).toBe(false);
  });
});

describe("HttpBodyCM", () => {
  it("keeps the extensions array stable across value changes (no flash)", () => {
    const onCommit = vi.fn();
    const { rerender } = renderWithProviders(
      <HttpBodyCM value="hello" onCommit={onCommit} />,
    );
    // Simulate a commit-on-blur that makes the parent re-emit `value`.
    rerender(<HttpBodyCM value="hello world" onCommit={onCommit} />);

    expect(recordedExtensions.length).toBeGreaterThanOrEqual(2);
    // Same reference on every render → CM never reconfigures the whole
    // extension set on commit. Before the fix `value` was a memo dep, so
    // this reference changed on every commit (the flash).
    const first = recordedExtensions[0];
    expect(recordedExtensions.every((e) => e === first)).toBe(true);
  });

  it("toggles JSON highlight through the compartment on JSON-ness flip", () => {
    const onCommit = vi.fn();
    const { rerender } = renderWithProviders(
      <HttpBodyCM value={'{"a":1}'} onCommit={onCommit} />,
    );
    // Mounted as JSON → the compartment effect dispatched a reconfigure.
    expect(fakeView.dispatch).toHaveBeenCalled();
    const afterMount = fakeView.dispatch.mock.calls.length;

    // Flip to non-JSON → another reconfigure dispatch, but the extension
    // array (asserted above) is untouched.
    rerender(<HttpBodyCM value="plain text" onCommit={onCommit} />);
    expect(fakeView.dispatch.mock.calls.length).toBeGreaterThan(afterMount);
  });
});
