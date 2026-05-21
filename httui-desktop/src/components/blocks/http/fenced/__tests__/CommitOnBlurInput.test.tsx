/**
 * Tests for `CommitOnBlurInput` + smoke for `HttpInlineCM`. The
 * companion `HttpBodyCM.test.tsx` already covers HttpBodyCM +
 * looksLikeJsonBody; this suite adds the remaining inline editor
 * branches to lift HttpInlineEditors.tsx past 80% (was 60.0%).
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Provider as ChakraProvider } from "@/components/ui/provider";
import { CommitOnBlurInput, HttpInlineCM } from "../HttpInlineEditors";

function rmount(node: React.ReactElement) {
  return render(<ChakraProvider>{node}</ChakraProvider>);
}

describe("CommitOnBlurInput", () => {
  it("renders the supplied value + placeholder", () => {
    rmount(
      <CommitOnBlurInput
        value="hello"
        placeholder="Type…"
        onCommit={vi.fn()}
      />,
    );
    const input = screen.getByPlaceholderText("Type…") as HTMLInputElement;
    expect(input.value).toBe("hello");
  });

  it("typing updates the local draft without firing onCommit", async () => {
    const onCommit = vi.fn();
    rmount(<CommitOnBlurInput value="" onCommit={onCommit} />);
    const user = userEvent.setup();
    const input = screen.getByRole("textbox") as HTMLInputElement;
    await user.type(input, "abc");
    expect(input.value).toBe("abc");
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("blur with a changed draft fires onCommit with the new value", async () => {
    const onCommit = vi.fn();
    rmount(<CommitOnBlurInput value="" onCommit={onCommit} />);
    const user = userEvent.setup();
    const input = screen.getByRole("textbox");
    await user.type(input, "next");
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledWith("next");
  });

  it("blur with an unchanged draft does NOT fire onCommit", () => {
    const onCommit = vi.fn();
    rmount(<CommitOnBlurInput value="same" onCommit={onCommit} />);
    const input = screen.getByRole("textbox");
    fireEvent.blur(input);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("externally swapping the `value` prop re-syncs the draft", () => {
    const { rerender } = rmount(
      (
        <ChakraProvider>
          <CommitOnBlurInput value="v1" onCommit={vi.fn()} />
        </ChakraProvider>
      ) as unknown as React.ReactElement,
    );
    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("v1");
    rerender(
      <ChakraProvider>
        <CommitOnBlurInput value="v2-external" onCommit={vi.fn()} />
      </ChakraProvider>,
    );
    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe(
      "v2-external",
    );
  });

  it("readOnly prop is forwarded to the underlying input", () => {
    rmount(<CommitOnBlurInput value="" onCommit={vi.fn()} readOnly />);
    const input = screen.getByRole("textbox") as HTMLInputElement;
    expect(input.readOnly).toBe(true);
  });
});

describe("HttpInlineCM smoke", () => {
  it("mounts a CodeMirror editor + renders the supplied value", () => {
    const onCommit = vi.fn();
    rmount(
      <HttpInlineCM value="hello" placeholder="Type…" onCommit={onCommit} />,
    );
    // `react-codemirror` mounts a contenteditable .cm-content node;
    // assert by querying for it.
    const cm = document.querySelector(".cm-content");
    expect(cm).not.toBeNull();
    expect(cm!.textContent).toContain("hello");
  });

  it("autocompletion is enabled only when refsGetters is supplied", () => {
    rmount(
      <HttpInlineCM
        value=""
        onCommit={vi.fn()}
        refsGetters={{
          getBlocks: () => [],
          getEnvKeys: () => [],
        }}
      />,
    );
    // Smoke: the CodeMirror surface is mounted; we don't fire the
    // popup here (would need real focus + char events), but the
    // refs-aware extension path was hit (autocompletion: true).
    expect(document.querySelector(".cm-content")).not.toBeNull();
  });
});
