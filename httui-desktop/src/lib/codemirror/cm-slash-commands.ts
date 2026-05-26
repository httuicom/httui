import {
  type CompletionContext,
  type CompletionResult,
  type Completion,
  type CompletionSection,
} from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";

import {
  getRegisteredBlockIcons,
  getRegisteredBlockSlashCommands,
} from "@/lib/blocks/block-registry";

// ── Sections ────────────────────────────────────────────────────────────────

const BASIC: CompletionSection = { name: "Basic blocks", rank: 0 };
const FORMAT: CompletionSection = { name: "Formatting", rank: 1 };
const EXEC: CompletionSection = { name: "Executable", rank: 2 };

// ── Lucide SVG paths (same paths used by react-icons/lu) ────────────────────

const ICON_SVGS: Record<string, string> = {
  h1: '<path d="M4 12h8"/><path d="M4 18V6"/><path d="M12 18V6"/><path d="m17 12 3-2v8"/>',
  h2: '<path d="M4 12h8"/><path d="M4 18V6"/><path d="M12 18V6"/><path d="M21 18h-4c0-4 4-3 4-6 0-1.5-2-2.5-4-1"/>',
  h3: '<path d="M4 12h8"/><path d="M4 18V6"/><path d="M12 18V6"/><path d="M17.5 10.5c1.7-1 3.5 0 3.5 1.5a2 2 0 0 1-2 2"/><path d="M17 17.5c2 1.5 4 .3 4-1.5a2 2 0 0 0-2-2"/>',
  "bullet-list":
    '<line x1="8" x2="21" y1="6" y2="6"/><line x1="8" x2="21" y1="12" y2="12"/><line x1="8" x2="21" y1="18" y2="18"/><line x1="3" x2="3.01" y1="6" y2="6"/><line x1="3" x2="3.01" y1="12" y2="12"/><line x1="3" x2="3.01" y1="18" y2="18"/>',
  "ordered-list":
    '<line x1="10" x2="21" y1="6" y2="6"/><line x1="10" x2="21" y1="12" y2="12"/><line x1="10" x2="21" y1="18" y2="18"/><path d="M4 6h1v4"/><path d="M4 10h2"/><path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1"/>',
  "task-list":
    '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="m9 12 2 2 4-4"/>',
  quote:
    '<path d="M17 6H3"/><path d="M21 12H8"/><path d="M21 18H8"/><path d="M3 12v6"/>',
  divider: '<line x1="2" x2="22" y1="12" y2="12"/>',
  code: '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>',
  table:
    '<path d="M12 3v18"/><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M3 9h18"/><path d="M3 15h18"/>',
  math: '<path d="M18 7V4H6l6 8-6 8h12v-3"/>',
  "math-block": '<path d="M18 7V4H6l6 8-6 8h12v-3"/>',
  diagram:
    '<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
  // Block-type icons (database/http/future) are merged in below from
  // `block-registry`, so adding a new block type doesn't require an
  // edit here.
  ...getRegisteredBlockIcons(),
};

/** Create a DOM SVG element for an icon type */
function createIconSvg(type: string): SVGElement | null {
  const paths = ICON_SVGS[type];
  if (!paths) return null;

  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "18");
  svg.setAttribute("height", "18");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("fill", "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "2");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("stroke-linejoin", "round");
  svg.innerHTML = paths;
  return svg;
}

// ── Commands ────────────────────────────────────────────────────────────────

interface SlashCommand {
  label: string;
  type: string;
  shortcut?: string;
  insert: string;
  cursorOffset?: number;
  section: CompletionSection;
}

const COMMANDS: SlashCommand[] = [
  // Basic blocks
  {
    label: "Heading 1",
    type: "h1",
    shortcut: "#",
    insert: "# ",
    section: BASIC,
  },
  {
    label: "Heading 2",
    type: "h2",
    shortcut: "##",
    insert: "## ",
    section: BASIC,
  },
  {
    label: "Heading 3",
    type: "h3",
    shortcut: "###",
    insert: "### ",
    section: BASIC,
  },
  {
    label: "Bulleted list",
    type: "bullet-list",
    shortcut: "-",
    insert: "- ",
    section: BASIC,
  },
  {
    label: "Numbered list",
    type: "ordered-list",
    shortcut: "1.",
    insert: "1. ",
    section: BASIC,
  },
  {
    label: "To-do list",
    type: "task-list",
    shortcut: "[]",
    insert: "- [ ] ",
    section: BASIC,
  },
  {
    label: "Quote",
    type: "quote",
    shortcut: ">",
    insert: "> ",
    section: BASIC,
  },

  // Formatting
  {
    label: "Divider",
    type: "divider",
    shortcut: "---",
    insert: "---\n",
    section: FORMAT,
  },
  {
    label: "Code block",
    type: "code",
    shortcut: "```",
    insert: "```\n\n```",
    cursorOffset: -4,
    section: FORMAT,
  },
  {
    label: "Table",
    type: "table",
    insert:
      "| Col 1 | Col 2 | Col 3 |\n| ----- | ----- | ----- |\n|       |       |       |\n",
    section: FORMAT,
  },
  {
    label: "Inline formula",
    type: "math",
    shortcut: "$",
    insert: "$x^2$",
    cursorOffset: -1,
    section: FORMAT,
  },
  {
    label: "Block formula",
    type: "math-block",
    shortcut: "$$",
    insert: "$$\nE = mc^2\n$$",
    cursorOffset: -3,
    section: FORMAT,
  },
  {
    label: "Mermaid diagram",
    type: "diagram",
    insert: "```mermaid\ngraph TD\n  A --> B\n```\n",
    section: FORMAT,
  },

  // Executable — block types contribute their slash entries via
  // `block-registry`, so adding a new block type doesn't require an
  // edit here (DB / HTTP / future).
  ...getRegisteredBlockSlashCommands(EXEC),
];

// ── Completion source ───────────────────────────────────────────────────────

function slashCompletionSource(
  context: CompletionContext,
): CompletionResult | null {
  const line = context.state.doc.lineAt(context.pos);
  const lineTextBefore = context.state.doc.sliceString(line.from, context.pos);

  const match = lineTextBefore.match(/^(\s*)\/([\w\s]*)$/);
  if (!match) return null;

  const prefix = match[1];
  const query = match[2].toLowerCase();
  const from = line.from + prefix.length;

  const filtered = COMMANDS.filter((cmd) =>
    cmd.label.toLowerCase().includes(query),
  );

  if (filtered.length === 0) return null;

  const options: Completion[] = filtered.map((cmd) => ({
    label: `/${cmd.label}`,
    displayLabel: cmd.label,
    type: cmd.type,
    detail: cmd.shortcut,
    section: cmd.section,
    apply: (
      view: EditorView,
      _completion: Completion,
      from: number,
      to: number,
    ) => {
      const insert = cmd.insert;
      view.dispatch({
        changes: { from, to, insert },
        selection: { anchor: from + insert.length + (cmd.cursorOffset ?? 0) },
      });
    },
  }));

  return { from, options, filter: false };
}

// ── addToOptions icon renderer (exported for MarkdownEditor) ────────────────

/** Renders Lucide SVG icons in autocomplete items via CM6 addToOptions API */
export const slashIconOption = {
  render(completion: Completion): HTMLElement | null {
    if (!completion.type || !ICON_SVGS[completion.type]) return null;
    const wrapper = document.createElement("span");
    wrapper.className = "cm-slash-icon";
    const svg = createIconSvg(completion.type);
    if (svg) wrapper.appendChild(svg);
    return wrapper;
  },
  position: 20, // before label (50) and detail (100)
};

// ── Theme ───────────────────────────────────────────────────────────────────

const slashMenuTheme = EditorView.theme({
  // Container
  ".cm-tooltip-autocomplete": {
    background: "var(--chakra-colors-bg) !important",
    border: "1px solid var(--chakra-colors-border) !important",
    borderRadius: "12px !important",
    boxShadow: "var(--chakra-shadows-lg) !important",
    padding: "6px !important",
    minWidth: "300px",
    maxWidth: "380px",
    maxHeight: "380px",
    overflow: "hidden auto !important",
  },
  // List
  ".cm-tooltip-autocomplete ul": {
    fontFamily: "var(--chakra-fonts-body) !important",
    fontSize: "14px !important",
    listStyle: "none",
    margin: "0",
    padding: "0",
  },
  // Section headers
  ".cm-tooltip-autocomplete .cm-completionSection": {
    padding: "8px 10px 6px !important",
    fontSize: "12px !important",
    fontWeight: "500 !important",
    color: "var(--chakra-colors-fg-subtle) !important",
    borderTop: "1px solid var(--chakra-colors-border) !important",
    marginTop: "4px !important",
  },
  ".cm-tooltip-autocomplete .cm-completionSection:first-child": {
    borderTop: "none !important",
    marginTop: "0 !important",
  },
  // Items
  ".cm-tooltip-autocomplete ul li": {
    padding: "7px 10px !important",
    display: "flex !important",
    alignItems: "center !important",
    gap: "12px !important",
    borderRadius: "4px !important",
    cursor: "pointer",
    margin: "0 !important",
    lineHeight: "1.5 !important",
    border: "none !important",
  },
  ".cm-tooltip-autocomplete ul li[aria-selected]": {
    background: "var(--chakra-colors-bg-subtle) !important",
  },
  // Hide CM6 default icon (we use addToOptions instead)
  ".cm-tooltip-autocomplete .cm-completionIcon": {
    display: "none !important",
  },
  // Custom SVG icon from addToOptions
  ".cm-slash-icon": {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "20px",
    height: "20px",
    flexShrink: "0",
    color: "var(--chakra-colors-fg)",
  },
  // Label
  ".cm-tooltip-autocomplete .cm-completionLabel": {
    fontSize: "14px !important",
    fontWeight: "400 !important",
    color: "var(--chakra-colors-fg) !important",
    flex: "1 !important",
  },
  // Shortcut
  ".cm-tooltip-autocomplete .cm-completionDetail": {
    fontSize: "12px !important",
    fontFamily: "var(--chakra-fonts-body) !important",
    fontStyle: "normal !important",
    color: "var(--chakra-colors-fg-subtle) !important",
    marginLeft: "auto !important",
    flexShrink: "0",
    opacity: "1 !important",
  },
  // Matched text
  ".cm-tooltip-autocomplete .cm-completionMatchedText": {
    textDecoration: "none !important",
    fontWeight: "600 !important",
    color: "var(--chakra-colors-fg) !important",
  },
  // Scrollbar
  ".cm-tooltip-autocomplete::-webkit-scrollbar": { width: "4px" },
  ".cm-tooltip-autocomplete::-webkit-scrollbar-track": {
    background: "transparent",
  },
  ".cm-tooltip-autocomplete::-webkit-scrollbar-thumb": {
    background: "var(--chakra-colors-border)",
    borderRadius: "2px",
  },
});

/** Export the completion source for combining with other sources */
export { slashCompletionSource };

/** Slash commands extension for CM6 — activates on "/" at line start */
export function slashCommands(): Extension {
  return [slashMenuTheme];
}
