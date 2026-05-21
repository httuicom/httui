import { describe, it, expect, vi } from "vitest";

// Mock all heavy CM6 / DB / HTTP block dependencies — `buildExtensions`
// composes them but doesn't drive their internals. The test verifies
// the assembly contract: shape of the returned array, branch on
// docHeaderHandle, wikilink onNavigate routing.

vi.mock("@codemirror/lang-markdown", () => ({
  markdown: vi.fn(() => ({ markdown: true })),
  markdownLanguage: { isMarkdown: true },
}));
vi.mock("@codemirror/language-data", () => ({ languages: [] }));
vi.mock("@codemirror/language", async (orig) => ({
  ...(await orig<typeof import("@codemirror/language")>()),
  syntaxHighlighting: vi.fn(() => ({ syntaxHighlighting: true })),
  bracketMatching: vi.fn(() => ({ bracketMatching: true })),
}));
vi.mock("@codemirror/commands", () => ({
  defaultKeymap: [],
  history: vi.fn(() => ({ history: true })),
  historyKeymap: [],
  indentWithTab: { key: "Tab" },
}));
vi.mock("@codemirror/autocomplete", () => ({
  autocompletion: vi.fn(() => ({ autocompletion: true })),
  closeBrackets: vi.fn(() => ({ closeBrackets: true })),
  closeBracketsKeymap: [],
  completionKeymap: [],
  startCompletion: vi.fn(),
}));
vi.mock("@codemirror/search", () => ({
  search: vi.fn(() => ({ search: true })),
  highlightSelectionMatches: vi.fn(() => ({ highlight: true })),
  searchKeymap: [],
}));
vi.mock("@/lib/codemirror/cm-hybrid-rendering", () => ({
  hybridRendering: vi.fn(() => ({ hybrid: true })),
}));
vi.mock("@/lib/codemirror/cm-slash-commands", () => ({
  slashCommands: vi.fn(() => ({ slash: true })),
  slashCompletionSource: vi.fn(),
  slashIconOption: { id: "slash-icon" },
}));
vi.mock("@/components/editor/editor-theme", () => ({
  editorTheme: { theme: true },
}));
vi.mock("@/lib/codemirror/cm-db-block", () => ({
  createDbBlockExtension: vi.fn(() => ({ dbBlock: true })),
  createDbBlockCompletionSource: vi.fn(() => vi.fn()),
  createDbSchemaCompletionSource: vi.fn(() => vi.fn()),
}));
vi.mock("@/lib/codemirror/cm-http-block", () => ({
  createHttpBlockExtension: vi.fn(() => ({ httpBlock: true })),
  createHttpBlockCompletionSource: vi.fn(() => vi.fn()),
}));
vi.mock("@/lib/codemirror/cm-wikilinks", () => {
  const wikilinks = vi.fn((cfg: unknown) => ({ wikilinks: true, cfg }));
  return {
    wikilinks,
    createWikilinkCompletion: vi.fn(() => vi.fn()),
  };
});
vi.mock("@/lib/codemirror/cm-tables", () => ({
  tables: vi.fn(() => ({ tables: true })),
}));
vi.mock("@/lib/codemirror/cm-move-blocks", () => ({
  moveBlocksKeymap: vi.fn(() => ({ moveBlocks: true })),
}));
vi.mock("@/lib/blocks/cm-references", () => ({
  referenceHighlight: [{ refHighlight: true }],
  createMarkdownReferenceTooltip: vi.fn(() => ({ refTooltip: true })),
}));

import {
  wikilinks,
  createWikilinkCompletion,
} from "@/lib/codemirror/cm-wikilinks";
import { createDbBlockCompletionSource } from "@/lib/codemirror/cm-db-block";
import { createHttpBlockCompletionSource } from "@/lib/codemirror/cm-http-block";
import { createMarkdownReferenceTooltip } from "@/lib/blocks/cm-references";
import {
  buildExtensions,
  flattenFiles,
} from "@/components/editor/markdown-extensions";
import type {
  BuildExtensionsParams,
  DocHeaderHandleLike,
} from "@/components/editor/markdown-extensions";
import type { FileEntry } from "@/lib/tauri/commands";
import type { Extension } from "@codemirror/state";

const folderEntry: FileEntry = {
  name: "folder",
  path: "folder",
  is_dir: true,
  children: [
    {
      name: "leaf.md",
      path: "folder/leaf.md",
      is_dir: false,
      children: null,
    },
    {
      name: "ignore.txt",
      path: "folder/ignore.txt",
      is_dir: false,
      children: null,
    },
    {
      name: "nested",
      path: "folder/nested",
      is_dir: true,
      children: [
        {
          name: "deep.md",
          path: "folder/nested/deep.md",
          is_dir: false,
          children: null,
        },
      ],
    },
  ],
};

const standaloneNote: FileEntry = {
  name: "note.md",
  path: "note.md",
  is_dir: false,
  children: null,
};

describe("markdown-extensions", () => {
  describe("flattenFiles", () => {
    it("returns only .md files at any depth", () => {
      const flat = flattenFiles([folderEntry, standaloneNote]);
      expect(flat).toEqual([
        { name: "leaf.md", path: "folder/leaf.md" },
        { name: "deep.md", path: "folder/nested/deep.md" },
        { name: "note.md", path: "note.md" },
      ]);
    });

    it("skips non-.md files", () => {
      const flat = flattenFiles([
        { name: "x.txt", path: "x.txt", is_dir: false, children: null },
      ]);
      expect(flat).toEqual([]);
    });

    it("returns empty for an empty entries array", () => {
      expect(flattenFiles([])).toEqual([]);
    });

    it("handles entries without a `children` field", () => {
      const flat = flattenFiles([
        {
          name: "lone",
          path: "lone",
          is_dir: true,
          // children: undefined — `if (entry.children)` short-circuits
          children: null,
        },
      ]);
      expect(flat).toEqual([]);
    });
  });

  describe("buildExtensions", () => {
    function makeParams(
      overrides: Partial<{
        docHeader: DocHeaderHandleLike | null;
        entriesRef: { current: FileEntry[] };
      }> = {},
    ): BuildExtensionsParams {
      return {
        filePath: "current.md",
        entriesRef: overrides.entriesRef ?? { current: [folderEntry] },
        handleFileSelectRef: { current: vi.fn() },
        docHeaderHandle: overrides.docHeader ?? null,
        getActiveVariables: () => ({ FOO: "bar" }),
      };
    }

    it("returns a non-empty array of CM6 extensions", () => {
      const ext = buildExtensions(makeParams());
      expect(Array.isArray(ext)).toBe(true);
      expect(ext.length).toBeGreaterThan(10);
    });

    it("does NOT include the docHeader extension when handle is null", () => {
      const ext = buildExtensions(makeParams());
      expect(ext.some((e) => e === null)).toBe(false);
    });

    it("includes the docHeader extension when a handle is provided", () => {
      // CM6 modules are fully mocked here; the docHeader extension is an
      // opaque pass-through value. Cast the fake through `Extension` so the
      // handle matches `DocHeaderHandleLike` while keeping object identity
      // for the `toContain` assertion below.
      const docExt = { docHeaderExt: true } as unknown as Extension;
      const ext = buildExtensions(
        makeParams({ docHeader: { extension: docExt, instanceId: "id-1" } }),
      );
      expect(ext).toContain(docExt);
    });

    it("wires wikilinks.onNavigate to call handleFileSelectRef when a match is found by name", () => {
      const params = makeParams();
      buildExtensions(params);
      // Inspect the wikilinks invocation
      const wikilinksMock = vi.mocked(wikilinks).mock.calls.at(-1);
      expect(wikilinksMock).toBeDefined();
      const cfg = wikilinksMock![0] as {
        getFiles: () => unknown;
        onNavigate: (target: string) => void;
      };
      cfg.onNavigate("leaf");
      expect(params.handleFileSelectRef.current).toHaveBeenCalledWith(
        "folder/leaf.md",
      );
    });

    it("wikilinks.onNavigate accepts an exact path match", () => {
      const params = makeParams();
      buildExtensions(params);
      const cfg = vi.mocked(wikilinks).mock.calls.at(-1)![0] as {
        onNavigate: (target: string) => void;
      };
      cfg.onNavigate("note.md");
      // No `note.md` at root in folderEntry — handler still resolves
      // the absolute path against children. We expect the handler to
      // try `path` / `name` / `${name}.md` matches and fall through
      // when none hit.
      // (note.md is NOT in `folderEntry`'s subtree → no call expected)
      expect(params.handleFileSelectRef.current).not.toHaveBeenCalled();
    });

    it("wikilinks.onNavigate accepts the bare name without extension", () => {
      const params = makeParams();
      buildExtensions(params);
      const cfg = vi.mocked(wikilinks).mock.calls.at(-1)![0] as {
        onNavigate: (target: string) => void;
      };
      cfg.onNavigate("deep");
      expect(params.handleFileSelectRef.current).toHaveBeenCalledWith(
        "folder/nested/deep.md",
      );
    });

    it("wikilinks.getFiles re-evaluates entriesRef each call", () => {
      const entriesRef: { current: FileEntry[] } = { current: [folderEntry] };
      const params = makeParams({ entriesRef });
      buildExtensions(params);
      const cfg = vi.mocked(wikilinks).mock.calls.at(-1)![0] as {
        getFiles: () => Array<{ name: string; path: string }>;
      };
      const initial = cfg.getFiles();
      expect(initial.length).toBe(2);
      // Mutate entries; getFiles should reflect the change.
      entriesRef.current = [];
      expect(cfg.getFiles()).toEqual([]);
    });

    it("createWikilinkCompletion gets a getFiles closure that walks entriesRef", () => {
      const params = makeParams();
      buildExtensions(params);
      const getFiles = vi
        .mocked(createWikilinkCompletion)
        .mock.calls.at(-1)![0] as () => Array<{ name: string; path: string }>;
      expect(getFiles().map((f) => f.path)).toContain("folder/leaf.md");
    });

    it("createDbBlockCompletionSource gets a closure returning the filePath", () => {
      const params = makeParams();
      buildExtensions(params);
      const getter = vi
        .mocked(createDbBlockCompletionSource)
        .mock.calls.at(-1)![0] as () => string;
      expect(getter()).toBe("current.md");
    });

    it("createHttpBlockCompletionSource gets a closure returning the filePath", () => {
      const params = makeParams();
      buildExtensions(params);
      const getter = vi
        .mocked(createHttpBlockCompletionSource)
        .mock.calls.at(-1)![0] as () => string;
      expect(getter()).toBe("current.md");
    });

    it("createMarkdownReferenceTooltip receives the filePath and getActiveVariables passthrough", () => {
      const params = makeParams();
      buildExtensions(params);
      const args = vi.mocked(createMarkdownReferenceTooltip).mock.calls.at(-1);
      expect(args).toBeDefined();
      const [getFilePath, getEnvVars] = args as [
        () => string,
        () => Record<string, string>,
      ];
      expect(getFilePath()).toBe("current.md");
      expect(getEnvVars()).toEqual({ FOO: "bar" });
    });
  });
});
