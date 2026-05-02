import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DocHeaderedEditor } from "@/components/layout/pane/DocHeaderedEditor";
import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { renderWithProviders, screen } from "@/test/render";

// V2 / cenário 4.5 — the DocHeader is now mounted INSIDE the
// MarkdownEditor via a CM6 block widget + React portal. The mock below
// reproduces that wiring at the integration boundary by rendering the
// inlineHeader's stub alongside the editor placeholder, so the existing
// `docheader-stub` assertions keep working without coupling them to
// CodeMirror itself.
function DocHeaderStub({
  filePath,
  compact,
  onToggleCompact,
  frontmatter,
  mtimeMs,
  dirty,
  branch,
  onTitleSave,
  onAbstractSave,
}: {
  filePath: string;
  compact?: boolean;
  onToggleCompact?: () => void;
  frontmatter?: {
    title?: string;
    abstract?: string;
    tags?: readonly string[];
  } | null;
  mtimeMs?: number | null;
  dirty?: boolean;
  branch?: { branch: string | null } | null;
  onTitleSave?: (title: string) => void;
  onAbstractSave?: (abstract: string) => void;
}) {
  return (
    <>
      <button
        data-testid="docheader-stub"
        data-file={filePath}
        data-compact={String(Boolean(compact))}
        data-fm-null={String(frontmatter === null)}
        data-fm-title={frontmatter?.title ?? ""}
        data-fm-abstract={frontmatter?.abstract ?? ""}
        data-fm-tags={(frontmatter?.tags ?? []).join(",")}
        data-mtime={String(mtimeMs ?? "")}
        data-dirty={String(Boolean(dirty))}
        data-branch={branch?.branch ?? ""}
        onClick={() => onToggleCompact?.()}
      >
        docheader
      </button>
      {/* Hook for the save-title flow: a test clicks this to drive
          `onTitleSave` with a fixed value, exercising the full
          `updateFrontmatterTitle` → `onChange` wiring without needing
          to render the real input + debounce machinery. */}
      {onTitleSave && (
        <button
          data-testid="docheader-stub-save-title"
          onClick={() => onTitleSave("Renamed")}
        >
          save title
        </button>
      )}
      {onAbstractSave && (
        <button
          data-testid="docheader-stub-save-abstract"
          onClick={() => onAbstractSave("New summary")}
        >
          save abstract
        </button>
      )}
    </>
  );
}

vi.mock("@/components/editor/MarkdownEditor", () => ({
  MarkdownEditor: ({
    filePath,
    content,
    inlineHeader,
  }: {
    filePath: string;
    content: string;
    inlineHeader?: Parameters<typeof DocHeaderStub>[0];
  }) => (
    <>
      {inlineHeader && <DocHeaderStub {...inlineHeader} />}
      <div
        data-testid="markdown-editor"
        data-file={filePath}
        data-len={content.length}
      />
    </>
  ),
}));

vi.mock("../../ConflictBanner", () => ({
  ConflictBanner: ({ filePath }: { filePath: string }) => (
    <div data-testid="conflict-banner" data-file={filePath} />
  ),
}));

beforeEach(() => {
  clearTauriMocks();
  // Default Tauri stubs that the hooks fetch on mount. Per-test
  // overrides via mockTauriCommand replace these.
  mockTauriCommand("get_file_mtime", () => 1_700_000_000_000);
  mockTauriCommand("git_status_cmd", () => ({
    branch: "main",
    upstream: null,
    ahead: 0,
    behind: 0,
    changed: [],
    clean: true,
  }));
});

afterEach(() => {
  clearTauriMocks();
});

describe("DocHeaderedEditor", () => {
  const baseProps = {
    filePath: "notes/foo.md",
    vaultPath: "/v",
    content: "# hi\n",
    vimEnabled: false,
    showConflict: false,
    dirty: false,
    onConflictReload: vi.fn(),
    onConflictKeep: vi.fn(),
    onChange: vi.fn(),
    onNavigateFile: undefined,
  };


  it("mounts the DocHeader card above the editor with filePath threaded", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    renderWithProviders(<DocHeaderedEditor {...baseProps} />);

    const header = screen.getByTestId("docheader-stub");
    const editor = screen.getByTestId("markdown-editor");
    expect(header).toBeInTheDocument();
    expect(editor).toBeInTheDocument();
    expect(header.dataset.file).toBe("notes/foo.md");
    expect(editor.dataset.file).toBe("notes/foo.md");
    // Header renders above the editor in document order so the layout
    // visually places it on top.
    expect(
      (header.compareDocumentPosition(editor) &
        Node.DOCUMENT_POSITION_FOLLOWING) >
        0,
    ).toBe(true);
  });

  it("hides the conflict banner unless showConflict is true", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    const { rerender } = renderWithProviders(
      <DocHeaderedEditor {...baseProps} />,
    );
    expect(screen.queryByTestId("conflict-banner")).not.toBeInTheDocument();

    rerender(<DocHeaderedEditor {...baseProps} showConflict />);
    expect(screen.getByTestId("conflict-banner")).toBeInTheDocument();
  });

  it("threads compact=false initially when the file has no override", async () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    renderWithProviders(<DocHeaderedEditor {...baseProps} />);
    const header = await screen.findByTestId("docheader-stub");
    expect(header.dataset.compact).toBe("false");
  });

  it("reflects compact=true when workspace.toml has the flag set", async () => {
    mockTauriCommand("get_file_settings", () => ({
      auto_capture: false,
      docheader_compact: true,
    }));
    renderWithProviders(<DocHeaderedEditor {...baseProps} />);
    // Wait for the hook's initial fetch settle by polling the prop.
    await vi.waitFor(() => {
      const header = screen.getByTestId("docheader-stub");
      expect(header.dataset.compact).toBe("true");
    });
  });

  it("flips compact + persists when the user clicks the title", async () => {
    let nextSettings: { auto_capture: boolean; docheader_compact?: boolean } = {
      auto_capture: false,
      docheader_compact: false,
    };
    mockTauriCommand("get_file_settings", () => nextSettings);
    const setCalls: Array<{ compact: boolean }> = [];
    mockTauriCommand("set_file_docheader_compact", (args) => {
      const a = args as { compact: boolean };
      setCalls.push(a);
      nextSettings = {
        auto_capture: false,
        docheader_compact: Boolean(a.compact),
      };
      return null;
    });

    renderWithProviders(<DocHeaderedEditor {...baseProps} />);
    const header = await screen.findByTestId("docheader-stub");
    header.click();

    await vi.waitFor(() => {
      expect(setCalls).toHaveLength(1);
      expect(setCalls[0]?.compact).toBe(true);
    });
  });

  it("renders the editor with the same filePath + content length", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    renderWithProviders(
      <DocHeaderedEditor {...baseProps} content={"abcd"} />,
    );
    const editor = screen.getByTestId("markdown-editor");
    expect(editor.dataset.file).toBe("notes/foo.md");
    expect(editor.dataset.len).toBe("4");
  });

  it("threads frontmatter=null when content has no fence", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    renderWithProviders(
      <DocHeaderedEditor {...baseProps} content={"# heading only\n"} />,
    );
    const header = screen.getByTestId("docheader-stub");
    expect(header.dataset.fmNull).toBe("true");
    expect(header.dataset.fmTitle).toBe("");
    expect(header.dataset.fmTags).toBe("");
  });

  it("parses frontmatter title + abstract + tags from the document", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    const doc = [
      "---",
      'title: "Payments — debug capture failures"',
      'abstract: "Capture flow when X"',
      "tags: [payments, debug]",
      "---",
      "body",
      "",
    ].join("\n");
    renderWithProviders(
      <DocHeaderedEditor {...baseProps} content={doc} />,
    );
    const header = screen.getByTestId("docheader-stub");
    expect(header.dataset.fmNull).toBe("false");
    expect(header.dataset.fmTitle).toBe("Payments — debug capture failures");
    expect(header.dataset.fmAbstract).toBe("Capture flow when X");
    expect(header.dataset.fmTags).toBe("payments,debug");
  });

  it("frontmatter survives subsequent body edits (parser memoized on content)", async () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    const initial = "---\ntitle: Stable\n---\nbody\n";
    const { rerender } = renderWithProviders(
      <DocHeaderedEditor {...baseProps} content={initial} />,
    );
    expect(screen.getByTestId("docheader-stub").dataset.fmTitle).toBe(
      "Stable",
    );

    // Edit body only — title stays.
    rerender(
      <DocHeaderedEditor
        {...baseProps}
        content={"---\ntitle: Stable\n---\nbody edited\n"}
      />,
    );
    expect(screen.getByTestId("docheader-stub").dataset.fmTitle).toBe(
      "Stable",
    );

    // Edit title — reflects.
    rerender(
      <DocHeaderedEditor
        {...baseProps}
        content={"---\ntitle: Renamed\n---\nbody edited\n"}
      />,
    );
    expect(screen.getByTestId("docheader-stub").dataset.fmTitle).toBe(
      "Renamed",
    );
  });

  it("threads mtime from useFileMtime into the meta strip", async () => {
    mockTauriCommand("get_file_mtime", () => 1_734_000_000_000);
    renderWithProviders(<DocHeaderedEditor {...baseProps} />);
    await vi.waitFor(() => {
      expect(screen.getByTestId("docheader-stub").dataset.mtime).toBe(
        "1734000000000",
      );
    });
  });

  it("threads dirty=true into the meta strip when the prop is set", () => {
    renderWithProviders(<DocHeaderedEditor {...baseProps} dirty={true} />);
    expect(screen.getByTestId("docheader-stub").dataset.dirty).toBe("true");
  });

  it("threads branch name from useGitStatus into the meta strip", async () => {
    mockTauriCommand("git_status_cmd", () => ({
      branch: "feat/integration",
      upstream: null,
      ahead: 0,
      behind: 0,
      changed: [],
      clean: true,
    }));
    renderWithProviders(<DocHeaderedEditor {...baseProps} />);
    await vi.waitFor(() => {
      expect(screen.getByTestId("docheader-stub").dataset.branch).toBe(
        "feat/integration",
      );
    });
  });

  it("leaves branch null when the git status hasn't loaded yet", () => {
    mockTauriCommand("git_status_cmd", () => {
      throw new Error("not a git repo");
    });
    renderWithProviders(<DocHeaderedEditor {...baseProps} />);
    expect(screen.getByTestId("docheader-stub").dataset.branch).toBe("");
  });

  it("refreshes mtime on the dirty → clean rising edge (post-save)", async () => {
    let mtimeValue = 1_700_000_000_000;
    let calls = 0;
    mockTauriCommand("get_file_mtime", () => {
      calls += 1;
      return mtimeValue;
    });
    const { rerender } = renderWithProviders(
      <DocHeaderedEditor {...baseProps} dirty={true} />,
    );
    await vi.waitFor(() => {
      expect(calls).toBeGreaterThanOrEqual(1);
    });
    const beforeRefresh = calls;

    // Simulate the post-save flip: dirty true → false should trigger
    // a fresh mtime poll. Bump the mtime so the value visibly
    // changes after refresh.
    mtimeValue = 1_700_000_999_999;
    rerender(<DocHeaderedEditor {...baseProps} dirty={false} />);

    await vi.waitFor(() => {
      expect(calls).toBeGreaterThan(beforeRefresh);
      expect(screen.getByTestId("docheader-stub").dataset.mtime).toBe(
        "1700000999999",
      );
    });
  });

  it("does NOT refresh mtime when dirty stays the same across rerenders", async () => {
    let calls = 0;
    mockTauriCommand("get_file_mtime", () => {
      calls += 1;
      return 1_700_000_000_000;
    });
    const { rerender } = renderWithProviders(
      <DocHeaderedEditor {...baseProps} dirty={false} />,
    );
    await vi.waitFor(() => expect(calls).toBeGreaterThanOrEqual(1));
    const initialCalls = calls;

    // Idempotent rerender — dirty stays false, no extra mtime poll.
    rerender(<DocHeaderedEditor {...baseProps} dirty={false} />);
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(calls).toBe(initialCalls);
  });

  it("rewrites the doc and fires onChange when the title is committed", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    const onChange = vi.fn();
    renderWithProviders(
      <DocHeaderedEditor
        {...baseProps}
        content={"---\ntitle: Old\n---\nbody\n"}
        onChange={onChange}
      />,
    );

    screen.getByTestId("docheader-stub-save-title").click();

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(
      "---\ntitle: Renamed\n---\nbody\n",
    );
  });

  it("rewrites the doc and fires onChange when the abstract is committed", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    const onChange = vi.fn();
    renderWithProviders(
      <DocHeaderedEditor
        {...baseProps}
        content={"---\ntitle: x\n---\nbody\n"}
        onChange={onChange}
      />,
    );

    screen.getByTestId("docheader-stub-save-abstract").click();

    expect(onChange).toHaveBeenCalledWith(
      "---\ntitle: x\nabstract: New summary\n---\nbody\n",
    );
  });

  it("prepends a new frontmatter when committing a title in virtual mode", () => {
    mockTauriCommand("get_file_settings", () => ({ auto_capture: false }));
    const onChange = vi.fn();
    renderWithProviders(
      <DocHeaderedEditor
        {...baseProps}
        content={"# Heading\n\nbody\n"}
        onChange={onChange}
      />,
    );

    screen.getByTestId("docheader-stub-save-title").click();

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(
      "---\ntitle: Renamed\n---\n\n# Heading\n\nbody\n",
    );
  });

  it("does NOT refresh mtime on the clean → dirty falling edge", async () => {
    let calls = 0;
    mockTauriCommand("get_file_mtime", () => {
      calls += 1;
      return 1_700_000_000_000;
    });
    const { rerender } = renderWithProviders(
      <DocHeaderedEditor {...baseProps} dirty={false} />,
    );
    await vi.waitFor(() => expect(calls).toBeGreaterThanOrEqual(1));
    const initialCalls = calls;

    // Editing flips dirty: clean → dirty. Mtime hasn't changed on
    // disk (write hasn't happened yet); skip the refresh.
    rerender(<DocHeaderedEditor {...baseProps} dirty={true} />);
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(calls).toBe(initialCalls);
  });
});
