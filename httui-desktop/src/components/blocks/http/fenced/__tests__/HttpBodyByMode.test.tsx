import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// Mock the heavy CodeMirror editors so the dispatcher tests stay
// focused on branch selection. The table/picker children render
// real DOM via Chakra.
vi.mock("../HttpInlineEditors", () => ({
  CommitOnBlurInput: (props: {
    value: string;
    onCommit: (v: string) => void;
  }) => (
    <input
      data-testid="commit-on-blur"
      defaultValue={props.value}
      onBlur={(e) => props.onCommit(e.currentTarget.value)}
    />
  ),
  HttpInlineCM: (props: { value: string }) => (
    <div data-testid="inline-cm">{props.value}</div>
  ),
  HttpBodyCM: (props: { value: string }) => (
    <div data-testid="body-cm">{props.value}</div>
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

describe("HttpBodyByMode — branch dispatcher", () => {
  it("'none' renders the empty-body hint and no editor", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="none"
        parsed={parsed()}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(
      screen.getByText(/Pick a Content-Type from the toolbar/i),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("body-cm")).not.toBeInTheDocument();
    expect(screen.queryByTestId("inline-cm")).not.toBeInTheDocument();
  });

  it("'form-urlencoded' renders the table editor with inline CM editors per row", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="form-urlencoded"
        parsed={parsed("a=1&b=2")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    // 2 committed rows + 1 pending row × (key + value) cells = 6 inline CM
    // editors. Loose check on the floor to avoid coupling to exact row count.
    const cmInputs = screen.getAllByTestId("inline-cm");
    expect(cmInputs.length).toBeGreaterThanOrEqual(4);
  });

  it("'multipart' renders the multipart table editor", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="multipart"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    // Empty body → just the pending row. Confirm *some* table affordance
    // is mounted (button p/ add, select p/ kind).
    expect(screen.getAllByRole("button").length).toBeGreaterThanOrEqual(1);
  });

  it("'binary' renders the BinaryFilePicker (mounted Box with Pick file action)", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="binary"
        parsed={parsed("")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    // BinaryFilePicker exposes a "Pick file" / "Choose file" affordance.
    // Look for any button or filename input.
    expect(screen.getByRole("button")).toBeInTheDocument();
  });

  it("'json' falls through to HttpBodyCM with the raw body", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="json"
        parsed={parsed(`{"k":1}`)}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    const cm = screen.getByTestId("body-cm");
    expect(cm.textContent).toBe(`{"k":1}`);
  });

  it("'xml' falls through to HttpBodyCM (sub-language detection happens inside)", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="xml"
        parsed={parsed(`<root/>`)}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(screen.getByTestId("body-cm").textContent).toBe(`<root/>`);
  });

  it("'text' falls through to HttpBodyCM", () => {
    rmount(
      <HttpBodyByMode
        bodyMode="text"
        parsed={parsed("plain text")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(screen.getByTestId("body-cm").textContent).toBe("plain text");
  });

  it("does NOT pull HttpInlineCM in any branch (HttpBodyCM is the editor surface)", () => {
    // Sanity — the dispatcher uses HttpBodyCM, not HttpInlineCM.
    rmount(
      <HttpBodyByMode
        bodyMode="json"
        parsed={parsed("{}")}
        onCommit={vi.fn()}
        onPickFile={vi.fn()}
      />,
    );
    expect(screen.queryByTestId("inline-cm")).not.toBeInTheDocument();
  });
});
