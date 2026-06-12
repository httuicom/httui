import { EditorView } from "@codemirror/view";

// Static theme for the markdown editor. Module-level constant so the
// CM6 EditorView only allocates a single instance — Emotion can cache
// the resulting CSS and the editor never re-themes on parent re-render.
//
// Method colors come from the Fuji oklch palette
// (`@/theme/tokens.ts METHOD_COLORS`); the `--chakra-colors-method-*`
// CSS vars are emitted by `lib/theme.ts` so the styles below can stay
// pure CSS.
export const editorTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "14px",
      color: "var(--chakra-colors-fg)",
      backgroundColor: "var(--chakra-colors-bg)",
    },
    "&.cm-editor > .cm-scroller > .cm-content": {
      // Markdown prose font (FONT_MARKDOWN_BODY → Latin Modern Roman).
      // Fence lines (HTTP/DB/code) override this with --chakra-fonts-mono;
      // numbered headings use --chakra-fonts-serif.
      fontFamily: "var(--chakra-fonts-markdown)",
      padding: "24px 32px",
      caretColor: "var(--chakra-colors-fg)",
      color: "var(--chakra-colors-fg)",
      overflow: "hidden",
    },
    ".cm-cursor": {
      borderLeftColor: "var(--chakra-colors-fg)",
    },
    ".cm-scroller": {
      overflow: "auto",
      overflowAnchor: "none",
    },
    ".cm-content": {
      overflowAnchor: "none",
    },
    ".cm-gutters": {
      display: "none",
    },
    ".cm-activeLine": {
      backgroundColor: "transparent",
    },
    ".tok-meta": {
      color: "var(--chakra-colors-fg-subtle) !important",
    },
    ".cm-selectionBackground, ::selection": {
      backgroundColor: "var(--chakra-colors-blue-500/20) !important",
    },
    ".cm-line:has(.tok-meta)": {
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "0.875em",
    },
    ".cm-vim-panel": {
      padding: "2px 8px",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "13px",
      backgroundColor: "var(--chakra-colors-bg-subtle)",
      borderTop: "1px solid var(--chakra-colors-border)",
      color: "var(--chakra-colors-fg)",
    },
    ".cm-vim-panel input": {
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "13px",
      backgroundColor: "transparent",
      color: "var(--chakra-colors-fg)",
      border: "none",
      outline: "none",
    },
    ".cm-searchMatch": {
      backgroundColor: "var(--chakra-colors-yellow-500/30)",
      borderRadius: "2px",
    },
    ".cm-searchMatch-selected": {
      backgroundColor: "var(--chakra-colors-yellow-500/50)",
    },
    ".cm-panels": {
      backgroundColor: "var(--chakra-colors-bg-subtle)",
      color: "var(--chakra-colors-fg)",
    },
    ".cm-panels-bottom": {
      borderTop: "1px solid var(--chakra-colors-border)",
    },

    // ── Autocomplete popup (shared by db blocks + slash + wikilinks) ──
    ".cm-tooltip.cm-tooltip-autocomplete": {
      border: "1px solid var(--chakra-colors-border)",
      backgroundColor: "var(--chakra-colors-bg)",
      borderRadius: "6px",
      boxShadow: "0 8px 24px rgba(0,0,0,0.35)",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "12px",
      overflow: "hidden",
      marginTop: "2px",
    },
    ".cm-tooltip.cm-tooltip-autocomplete > ul": {
      maxHeight: "260px",
      maxWidth: "360px",
      minWidth: "200px",
      fontFamily: "inherit",
    },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li": {
      padding: "3px 10px",
      lineHeight: "1.4",
      display: "flex",
      alignItems: "center",
      gap: "6px",
      color: "var(--chakra-colors-fg)",
    },
    ".cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]": {
      backgroundColor: "var(--chakra-colors-bg-subtle)",
      color: "var(--chakra-colors-fg)",
    },
    ".cm-completionLabel": {
      flex: "1",
      minWidth: 0,
      whiteSpace: "nowrap",
      overflow: "hidden",
      textOverflow: "ellipsis",
    },
    ".cm-completionMatchedText": {
      color: "var(--chakra-colors-brand-400)",
      textDecoration: "none",
      fontWeight: "600",
    },
    ".cm-completionDetail": {
      color: "var(--chakra-colors-fg-muted)",
      fontStyle: "normal",
      fontSize: "11px",
      marginLeft: "8px",
      flexShrink: 0,
    },
    ".cm-completionIcon": { display: "none" },
    ".cm-block-portal": {
      overflowAnchor: "none",
      width: "100%",
      background: "var(--chakra-colors-bg)",
      padding: "8px 0",
      borderRadius: "8px",
    },
    ".cm-hidden-block-line": {
      height: "0 !important",
      padding: "0 !important",
      margin: "0 !important",
      overflow: "hidden !important",
      fontSize: "0 !important",
      lineHeight: "0 !important",
      border: "none !important",
    },

    // ── db block SQL error squiggle ──
    ".cm-db-sql-error": {
      textDecoration: "underline wavy var(--chakra-colors-red-400)",
      textDecorationThickness: "1px",
      textUnderlineOffset: "2px",
      backgroundColor: "var(--chakra-colors-red-500/10)",
      borderRadius: "2px",
    },

    // ── db block (unified slab card) ──
    ".cm-db-fence-line": {
      color: "var(--chakra-colors-fg-muted)",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "var(--chakra-font-sizes-xs)",
      opacity: 0.3,
      position: "relative",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      background: "var(--chakra-colors-bg-subtle)",
      paddingLeft: "var(--chakra-spacing-4)",
      paddingRight: "var(--chakra-spacing-4)",
    },
    ".cm-db-fence-line-open": {
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTopLeftRadius: "var(--chakra-radii-md)",
      borderTopRightRadius: "var(--chakra-radii-md)",
      paddingTop: "var(--chakra-spacing-2)",
    },
    ".cm-db-fence-line-close": {
      paddingBottom: "var(--chakra-spacing-2)",
      borderBottom:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
    },

    ".cm-db-body-line": {
      fontFamily: "var(--chakra-fonts-mono)",
      background: "var(--chakra-colors-bg-canvas, var(--chakra-colors-bg))",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      paddingLeft: "44px",
      paddingRight: "var(--chakra-spacing-3)",
      paddingTop: 0,
      paddingBottom: 0,
      fontSize: "13px",
      lineHeight: "20px",
      position: "relative",
      counterIncrement: "db-line",
    },
    ".cm-db-body-line::before": {
      content: "counter(db-line)",
      position: "absolute",
      left: "var(--chakra-spacing-2)",
      top: 0,
      width: "20px",
      textAlign: "right",
      color: "var(--chakra-colors-fg-muted)",
      opacity: 0.5,
      fontSize: "inherit",
      lineHeight: "inherit",
      fontFamily: "var(--chakra-fonts-mono)",
      fontVariantNumeric: "tabular-nums",
      userSelect: "none",
      pointerEvents: "none",
    },
    ".cm-db-body-line-first": {
      counterReset: "db-line",
    },
    ".cm-db-fence-hidden": {
      height: 0,
      margin: 0,
      padding: 0,
    },
    ".cm-db-close-panel": {
      display: "block",
      paddingBottom: "var(--chakra-spacing-4)",
    },

    // ── DB toolbar widget (card header) ──
    ".cm-db-toolbar-portal": {
      display: "block",
      background:
        "color-mix(in srgb, var(--chakra-colors-fg) 2.5%, var(--chakra-colors-bg-subtle))",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTopLeftRadius: "var(--chakra-radii-md)",
      borderTopRightRadius: "var(--chakra-radii-md)",
      paddingTop: "var(--chakra-spacing-1)",
      paddingBottom: "var(--chakra-spacing-1)",
      paddingLeft: "var(--chakra-spacing-3)",
      paddingRight: "var(--chakra-spacing-3)",
      minHeight: "var(--chakra-spacing-8)",
      userSelect: "none",
      pointerEvents: "auto",
    },

    ".cm-db-result-portal": {
      overflowAnchor: "none",
      margin: 0,
      background: "var(--chakra-colors-bg)",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      minHeight: "var(--chakra-spacing-12)",
    },

    ".cm-db-statusbar-portal": {
      paddingTop: "var(--chakra-spacing-3)",
      paddingBottom: "var(--chakra-spacing-3)",
      paddingLeft: "var(--chakra-spacing-4)",
      paddingRight: "var(--chakra-spacing-4)",
      background:
        "color-mix(in srgb, var(--chakra-colors-fg) 1.5%, transparent)",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderBottom:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 40%, transparent)",
      borderBottomLeftRadius: "var(--chakra-radii-md)",
      borderBottomRightRadius: "var(--chakra-radii-md)",
      minHeight: "var(--chakra-spacing-9)",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "var(--chakra-font-sizes-xs)",
    },

    // ── HTTP block portals (mirror DB block styling) ──
    ".cm-http-toolbar-portal": {
      display: "block",
      background:
        "color-mix(in srgb, var(--chakra-colors-fg) 2.5%, var(--chakra-colors-bg-subtle))",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTopLeftRadius: "var(--chakra-radii-md)",
      borderTopRightRadius: "var(--chakra-radii-md)",
      minHeight: "var(--chakra-spacing-8)",
      userSelect: "none",
      pointerEvents: "auto",
    },
    ".cm-http-result-portal": {
      overflowAnchor: "none",
      margin: 0,
      background: "var(--chakra-colors-bg)",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      minHeight: "var(--chakra-spacing-12)",
      overscrollBehavior: "contain",
      "& [data-overflow='auto'], & pre, & .cm-scroller": {
        overscrollBehavior: "contain",
      },
      "& .hljs-attr": {
        color: "var(--chakra-colors-blue-500)",
      },
      "& .hljs-string": {
        color: "var(--chakra-colors-green-500)",
      },
      "& .hljs-number": {
        color: "var(--chakra-colors-orange-500)",
      },
      "& .hljs-literal, & .hljs-built_in": {
        color: "var(--chakra-colors-red-400)",
      },
      "& .hljs-keyword, & .hljs-selector-tag": {
        color: "var(--chakra-colors-purple-500)",
      },
      "& .hljs-punctuation, & .hljs-meta": {
        color: "var(--chakra-colors-fg-muted)",
      },
      "& .hljs-comment": {
        color: "var(--chakra-colors-fg-muted)",
        fontStyle: "italic",
      },
      "& .hljs-type, & .hljs-class .hljs-title": {
        color: "var(--chakra-colors-cyan-500)",
      },
      "& .hljs-tag, & .hljs-name, & .hljs-selector-id, & .hljs-selector-class":
        {
          color: "var(--chakra-colors-purple-400)",
        },
      "& .hljs-title": { color: "var(--chakra-colors-yellow-500)" },
    },
    ".cm-http-statusbar-portal": {
      background:
        "color-mix(in srgb, var(--chakra-colors-fg) 1.5%, transparent)",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderBottom:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 40%, transparent)",
      borderBottomLeftRadius: "var(--chakra-radii-md)",
      borderBottomRightRadius: "var(--chakra-radii-md)",
      minHeight: "var(--chakra-spacing-7)",
    },
    ".cm-http-form-portal": {
      display: "block",
      background: "var(--chakra-colors-bg-canvas, var(--chakra-colors-bg))",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      minHeight: "var(--chakra-spacing-12)",
    },
    ".cm-http-fence-hidden": { display: "none" },
    ".cm-http-body-line": {
      paddingLeft: "44px",
      paddingRight: "var(--chakra-spacing-3)",
      paddingTop: 0,
      paddingBottom: 0,
      background: "var(--chakra-colors-bg-canvas, var(--chakra-colors-bg))",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "13px",
      lineHeight: "20px",
      position: "relative",
      counterIncrement: "http-line",
      color: "var(--chakra-colors-fg)",
    },
    ".cm-http-body-line::before": {
      content: "counter(http-line)",
      position: "absolute",
      left: "var(--chakra-spacing-2)",
      top: 0,
      width: "20px",
      textAlign: "right",
      color: "var(--chakra-colors-fg-muted)",
      opacity: 0.5,
      fontSize: "inherit",
      lineHeight: "inherit",
      fontFamily: "var(--chakra-fonts-mono)",
      fontVariantNumeric: "tabular-nums",
      userSelect: "none",
      pointerEvents: "none",
    },
    ".cm-http-body-line-first": { counterReset: "http-line" },
    ".cm-http-fence-line": {
      color: "var(--chakra-colors-fg-muted)",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "var(--chakra-font-sizes-xs)",
      opacity: 0.3,
      position: "relative",
      background: "var(--chakra-colors-bg-subtle)",
      borderLeft:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderRight:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      paddingLeft: "var(--chakra-spacing-3)",
      paddingRight: "var(--chakra-spacing-3)",
    },
    ".cm-http-fence-line-open": {
      borderTop:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderTopLeftRadius: "var(--chakra-radii-md)",
      borderTopRightRadius: "var(--chakra-radii-md)",
      paddingTop: "var(--chakra-spacing-2)",
    },
    ".cm-http-fence-line-close": {
      paddingBottom: "var(--chakra-spacing-2)",
      borderBottom:
        "1px solid color-mix(in srgb, var(--chakra-colors-border) 55%, transparent)",
      borderBottomLeftRadius: "var(--chakra-radii-md)",
      borderBottomRightRadius: "var(--chakra-radii-md)",
    },

    // ── Numbered section headings (epic 39 / story 05) ──
    // The CM6 extension adds `.cm-numbered-heading` + a
    // `data-heading-number` attribute on every `#`/`##` line that
    // isn't inside a fence. Style: serif title with a small accent
    // circle carrying the number on the leading side.
    ".cm-numbered-heading": {
      position: "relative",
      paddingLeft: "32px",
      fontFamily: "var(--chakra-fonts-serif)",
    },
    // H1 typography per canvas spec (epic 40 / story 04):
    // 2.25rem weight 600. H2 keeps the line-default size — it inherits
    // weight 600 from `tags.heading` in MarkdownEditor's highlight style.
    '.cm-numbered-heading[data-heading-level="1"]': {
      fontSize: "2.25rem",
      fontWeight: 600,
      lineHeight: 1.05,
    },
    ".cm-numbered-heading::before": {
      content: "attr(data-heading-number)",
      position: "absolute",
      left: 0,
      top: "50%",
      transform: "translateY(-50%)",
      width: "20px",
      height: "20px",
      borderRadius: "9999px",
      background: "var(--chakra-colors-accent)",
      color: "var(--chakra-colors-accent-fg)",
      fontFamily: "var(--chakra-fonts-mono)",
      fontSize: "10px",
      fontWeight: 700,
      lineHeight: 1,
      display: "inline-flex",
      alignItems: "center",
      justifyContent: "center",
      userSelect: "none",
      pointerEvents: "none",
    },
  },
  { dark: true },
);
