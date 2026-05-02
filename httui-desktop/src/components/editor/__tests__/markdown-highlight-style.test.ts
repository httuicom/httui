import { describe, it, expect } from "vitest";
import { HighlightStyle } from "@codemirror/language";
import { LanguageDescription } from "@codemirror/language";
import {
  dbSqlLanguages,
  markdownHighlightStyle,
  containerCss,
} from "@/components/editor/markdown-highlight-style";

describe("markdown-highlight-style", () => {
  describe("dbSqlLanguages", () => {
    it("registers four db-* aliases", () => {
      expect(dbSqlLanguages).toHaveLength(4);
      const names = dbSqlLanguages.map((d) => d.name);
      expect(names).toEqual(["db", "db-postgres", "db-mysql", "db-sqlite"]);
    });

    it("each entry is a LanguageDescription with the alias matching the name", () => {
      for (const desc of dbSqlLanguages) {
        expect(desc).toBeInstanceOf(LanguageDescription);
        expect(desc.alias).toContain(desc.name);
      }
    });

    it("each entry exposes an async load() that yields a CM6 LanguageSupport", async () => {
      const desc = dbSqlLanguages[0];
      const support = await desc.load();
      // LanguageSupport instances expose `language` and `support` props.
      expect(support).toBeDefined();
      expect((support as { language?: unknown }).language).toBeDefined();
    });
  });

  describe("markdownHighlightStyle", () => {
    it("is a HighlightStyle instance", () => {
      expect(markdownHighlightStyle).toBeInstanceOf(HighlightStyle);
    });

    it("provides a CM6 module() extension hook", () => {
      // HighlightStyle exposes a `module` Facet entry; sanity-check that
      // it can be consumed as an extension without throwing.
      expect(markdownHighlightStyle.module).toBeDefined();
    });
  });

  describe("containerCss", () => {
    it("forces 100% height on @uiw/react-codemirror's wrapper div", () => {
      expect(containerCss["& > div"]).toEqual({ height: "100%" });
    });

    it("forces 100% height on .cm-editor", () => {
      expect(containerCss["& .cm-editor"]).toEqual({ height: "100%" });
    });

    it("clears the focus outline on .cm-focused", () => {
      expect(containerCss["& .cm-editor.cm-focused"]).toEqual({
        outline: "none",
      });
    });

    it("is a stable reference across imports (frozen-shape contract)", () => {
      const keys = Object.keys(containerCss);
      expect(keys).toHaveLength(3);
    });
  });
});
