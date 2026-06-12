// Nested-language registration for ```http fences.
//
// The Lezer grammar (@httui/lezer-http) drives syntax coloring of the
// fence body via markdown's `codeLanguages` injection — the same
// mechanism `dbSqlLanguages` uses for SQL. The body after the blank
// line is additionally delegated to the JSON/XML parser when the
// block's Content-Type header says so (parseMixed). The delegation
// lives here, in the consumer, so the published grammar package stays
// a pure lexical mirror of the canonical tree-sitter grammar.

import {
  LRLanguage,
  LanguageDescription,
  LanguageSupport,
} from "@codemirror/language";
import {
  parseMixed,
  type Input,
  type Parser,
  type SyntaxNodeRef,
} from "@lezer/common";

/** Read the enabled Content-Type header value from the fence's parse
 * tree. Disabled headers (`# Content-Type: ...`) are different node
 * types and therefore never match — same contract as the form-mode
 * body pill. */
export function bodyContentType(
  body: SyntaxNodeRef,
  input: Input,
): string | null {
  const root = body.node.parent;
  if (!root) return null;
  for (let line = root.firstChild; line; line = line.nextSibling) {
    if (line.from >= body.from) break;
    if (line.name !== "HeaderLine") continue;
    const name = line.node.getChild("HeaderName");
    const value = line.node.getChild("HeaderValue");
    if (!name || !value) continue;
    if (input.read(name.from, name.to).trim().toLowerCase() !== "content-type")
      continue;
    return input.read(value.from, value.to).trim();
  }
  return null;
}

/** Map a Content-Type value (parameters stripped) to the nested body
 * language. Only textual structured types get a parser — everything
 * else stays opaque. */
export function nestedModeFor(contentType: string): "json" | "xml" | null {
  const mime = contentType.split(";")[0].trim().toLowerCase();
  if (mime === "application/json" || mime.endsWith("+json")) return "json";
  if (
    mime === "application/xml" ||
    mime === "text/xml" ||
    mime.endsWith("+xml")
  )
    return "xml";
  return null;
}

export const httpLanguages: LanguageDescription[] = [
  LanguageDescription.of({
    name: "http",
    alias: ["http"],
    async load() {
      const [{ parser }, { jsonLanguage }, { xmlLanguage }] = await Promise.all(
        [
          import("@httui/lezer-http"),
          import("@codemirror/lang-json"),
          import("@codemirror/lang-xml"),
        ],
      );
      const nested: Record<string, Parser> = {
        json: jsonLanguage.parser,
        xml: xmlLanguage.parser,
      };
      const wrapped = parser.configure({
        wrap: parseMixed((node, input) => {
          if (node.name !== "Body") return null;
          const contentType = bodyContentType(node, input);
          if (!contentType) return null;
          const mode = nestedModeFor(contentType);
          return mode ? { parser: nested[mode] } : null;
        }),
      });
      const language = LRLanguage.define({
        name: "http",
        parser: wrapped,
      });
      return new LanguageSupport(language);
    },
  }),
];
