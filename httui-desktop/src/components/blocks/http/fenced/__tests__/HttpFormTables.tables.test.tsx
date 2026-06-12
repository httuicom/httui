// Interaction-test coverage for HttpFormTables internal components:
// FormUrlEncodedTable, MultipartTable, BinaryFilePicker.
//
// The dispatcher (HttpBodyByMode) is covered in HttpBodyByMode.test.tsx;
// pure helpers (parseUrlEncoded / stringifyUrlEncoded) in
// HttpFormTables.test.ts. This file exercises the table editors via
// `bodyMode` props so the internal components mount with realistic state.
//
// Coverage gate alvo: HttpFormTables 36.4% → ≥80%.

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Mock the heavy CodeMirror editors so we can assert commit calls cheaply.
// `HttpInlineCM` becomes a plain <input commit-on-blur>; `CommitOnBlurInput`
// already mirrors that shape but with a real ref to the DOM input.
vi.mock("../HttpInlineEditors", () => ({
  CommitOnBlurInput: (props: {
    placeholder?: string;
    value: string;
    onCommit: (v: string) => void;
  }) => (
    <input
      data-testid="commit-input"
      placeholder={props.placeholder}
      defaultValue={props.value}
      onBlur={(e) => props.onCommit(e.currentTarget.value)}
    />
  ),
  HttpInlineCM: (props: {
    placeholder?: string;
    value: string;
    onCommit: (v: string) => void;
  }) => (
    <input
      data-testid="inline-cm"
      placeholder={props.placeholder}
      defaultValue={props.value}
      onBlur={(e) => props.onCommit(e.currentTarget.value)}
    />
  ),
  HttpBodyCM: (p: { value: string }) => (
    <div data-testid="body-cm">{p.value}</div>
  ),
}));

import { Provider as ChakraProvider } from "@/components/ui/provider";
import { HttpBodyByMode } from "../HttpFormTables";
import type { HttpMessageParsed } from "@/lib/blocks/http-message";

function rmount(ui: React.ReactElement) {
  return render(<ChakraProvider>{ui}</ChakraProvider>);
}

const parsed = (body = ""): HttpMessageParsed => ({
  method: "POST",
  url: "https://api.example.com",
  params: [],
  headers: [],
  body,
});

// ─────────────────────── FormUrlEncodedTable ───────────────────────

describe("FormUrlEncodedTable", () => {
  it("renders empty-state hint when body has no rows", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(
      screen.getByText(/no fields — application\/x-www-form-urlencoded/i),
    ).toBeInTheDocument();
  });

  it("renders one row per committed key=value pair (+ pending row not shown until added)", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1&b=2")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    // 2 rows × (key + value) = 4 inline editors.
    const inputs = screen.getAllByTestId("inline-cm");
    expect(inputs.length).toBe(4);
  });

  it("clicking '+ add field' appends a pending row (extra 2 inputs)", async () => {
    const user = userEvent.setup();
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(screen.getAllByTestId("inline-cm").length).toBe(2);
    await user.click(screen.getByRole("button", { name: /\+ add field/i }));
    // 1 committed row + 1 pending row → 4 inputs.
    expect(screen.getAllByTestId("inline-cm").length).toBe(4);
  });

  it("editing an existing row commits a new stringified body", () => {
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    const [, valueInput] = screen.getAllByTestId("inline-cm");
    fireEvent.blur(valueInput, { target: { value: "99" } });
    // The mock invokes onCommit("99"); the row's updateRow then calls
    // stringifyUrlEncoded(['a','99']) → "a=99".
    expect(onCommit).toHaveBeenCalled();
    expect(onCommit.mock.calls.at(-1)?.[0]).toBe("a=99");
  });

  it("typing a key into a pending row promotes it (calls onCommit with merged body)", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add field/i }));
    // After adding: order is [committed-key, committed-value, pending-key, pending-value]
    const inputs = screen.getAllByTestId("inline-cm");
    const pendingKey = inputs[2];
    fireEvent.blur(pendingKey, { target: { value: "b" } });
    expect(onCommit).toHaveBeenCalled();
    // Promoted row keeps empty value; stringifyUrlEncoded ignores it →
    // "a=1&b".
    expect(onCommit.mock.calls.at(-1)?.[0]).toBe("a=1&b");
  });

  it("deleting a committed row commits the slimmer body", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1&b=2")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByLabelText("Delete field 0"));
    expect(onCommit).toHaveBeenLastCalledWith("b=2");
  });

  it("deleting a pending row only updates local state (no onCommit)", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add field/i }));
    onCommit.mockClear();
    await user.click(screen.getByLabelText("Delete field 1")); // pending row
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("editing a pending row with empty key keeps it pending (no commit)", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add field/i }));
    onCommit.mockClear();
    const inputs = screen.getAllByTestId("inline-cm");
    fireEvent.blur(inputs[3], { target: { value: "value-only" } });
    // Pending row still has empty key → no promotion, no commit.
    expect(onCommit).not.toHaveBeenCalled();
  });
});

// ─────────────────────── MultipartTable ───────────────────────

describe("MultipartTable", () => {
  it("renders empty-state hint when body has no parts", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(
      screen.getByText(/no parts — multipart\/form-data/i),
    ).toBeInTheDocument();
  });

  it("'+ add text part' creates a pending row with name + value inputs", async () => {
    const user = userEvent.setup();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add text part/i }));
    // 2 CommitOnBlurInput (name + value) on the new pending row.
    expect(screen.getAllByTestId("commit-input").length).toBe(2);
  });

  it("'+ add file part' creates a pending row with a Choose… button (no value input)", async () => {
    const user = userEvent.setup();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add file part/i }));
    expect(screen.getByRole("button", { name: /Choose…/ })).toBeInTheDocument();
    // Only the name input remains as a CommitOnBlurInput.
    expect(screen.getAllByTestId("commit-input").length).toBe(1);
  });

  it("toggling the part checkbox commits an updated body (when row is committed)", async () => {
    // Need a body with one part so that we mount a *committed* row whose
    // updatePart goes through the commit path (not the pending guard).
    // Use a minimal multipart body with a single boundary-wrapped part.
    const body =
      '--bx\r\nContent-Disposition: form-data; name="k"\r\n\r\nv\r\n--bx--\r\n';
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed(body)}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    const cb = screen.getByLabelText("Toggle part 0") as HTMLInputElement;
    fireEvent.click(cb);
    // onCommit fires with a new stringified body.
    expect(onCommit).toHaveBeenCalled();
  });

  it("deleting a pending part only updates local state", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add text part/i }));
    onCommit.mockClear();
    await user.click(screen.getByLabelText("Delete part 0"));
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("typing a name on a pending part promotes it (calls onCommit)", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add text part/i }));
    const nameInput = screen.getAllByTestId("commit-input")[0];
    fireEvent.blur(nameInput, { target: { value: "field" } });
    expect(onCommit).toHaveBeenCalled();
  });

  it("editing a pending part with empty name keeps it pending (no commit)", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add text part/i }));
    onCommit.mockClear();
    // Edit the value field (index 1) — name remains "" so part stays pending.
    const inputs = screen.getAllByTestId("commit-input");
    fireEvent.blur(inputs[1], { target: { value: "hello" } });
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("file picker (onPickFile resolves to a path) populates the pending row", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    const onPickFile = vi.fn(async () => "/tmp/foo.bin");
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={onPickFile}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add file part/i }));
    await user.click(screen.getByRole("button", { name: /Choose…/ }));
    expect(onPickFile).toHaveBeenCalled();
    // Pending row still has empty name → onCommit not invoked (just file).
    // The path is displayed in the row label.
    expect(screen.getByTitle("/tmp/foo.bin")).toBeInTheDocument();
  });

  it("file picker that returns null is a no-op", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    const onPickFile = vi.fn(async () => null);
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={onPickFile}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add file part/i }));
    await user.click(screen.getByRole("button", { name: /Choose…/ }));
    // Path display still shows the no-file placeholder.
    expect(screen.getByText(/no file selected/i)).toBeInTheDocument();
  });

  it("changing the kind select on a pending row updates it without commit", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add text part/i }));
    onCommit.mockClear();
    const select = screen.getByRole("combobox") as HTMLSelectElement;
    await user.selectOptions(select, "file");
    // Pending row → no commit; we just verify it switched to file mode
    // (Choose… button now appears).
    expect(screen.getByRole("button", { name: /Choose…/ })).toBeInTheDocument();
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("selecting the same kind is a no-op (early return)", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={vi.fn()}
      />,
    );
    await user.click(screen.getByRole("button", { name: /\+ add text part/i }));
    const select = screen.getByRole("combobox") as HTMLSelectElement;
    // Already "text"; selecting again should hit the `nextKind === part.kind`
    // early-return branch.
    await user.selectOptions(select, "text");
    expect(onCommit).not.toHaveBeenCalled();
  });
});

// ─────────────────────── BinaryFilePicker ───────────────────────

describe("BinaryFilePicker", () => {
  it("renders 'Choose…' + 'no file selected' when body is empty", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="binary"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(screen.getByText(/no file selected/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Choose…/ })).toBeInTheDocument();
  });

  it("file picker success → onCommit with buildBinaryFileBody string", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    const onPickFile = vi.fn(async () => "/abs/path/file.bin");
    rmount(
      <HttpBodyByMode
        bodyMode="binary"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={onPickFile}
      />,
    );
    await user.click(screen.getByRole("button", { name: /Choose…/ }));
    expect(onPickFile).toHaveBeenCalled();
    // Exact serialization is owned by buildBinaryFileBody (lives in
    // http-body-modes and has its own tests). Here we just assert onCommit
    // fired with the path embedded somewhere in the resulting string.
    expect(onCommit).toHaveBeenCalled();
    expect(String(onCommit.mock.calls[0][0])).toContain("/abs/path/file.bin");
  });

  it("file picker returning null is a no-op", async () => {
    const user = userEvent.setup();
    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="binary"
        parsed={parsed("")}
        onCommit={onCommit}
        onPickFile={vi.fn(async () => null)}
      />,
    );
    await user.click(screen.getByRole("button", { name: /Choose…/ }));
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("when a binary body is present: 'Replace…' + 'Clear' both work", async () => {
    const user = userEvent.setup();
    // The body must satisfy isBinaryFileBody → use the canonical token.
    // Easiest: build via buildBinaryFileBody indirectly: first set a body
    // via Choose, then re-render with the captured body.
    const onCommitPhase1 = vi.fn();
    const { unmount } = rmount(
      <HttpBodyByMode
        bodyMode="binary"
        parsed={parsed("")}
        onCommit={onCommitPhase1}
        onPickFile={vi.fn(async () => "/tmp/x.bin")}
      />,
    );
    await user.click(screen.getByRole("button", { name: /Choose…/ }));
    const binaryBody = String(onCommitPhase1.mock.calls[0][0]);
    unmount();

    const onCommit = vi.fn();
    rmount(
      <HttpBodyByMode
        bodyMode="binary"
        parsed={parsed(binaryBody)}
        onCommit={onCommit}
        onPickFile={vi.fn(async () => "/tmp/y.bin")}
      />,
    );
    expect(
      screen.getByRole("button", { name: /Replace…/ }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Clear/ }));
    expect(onCommit).toHaveBeenCalledWith("");
  });
});
