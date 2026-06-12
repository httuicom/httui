// Markdown CM6 highlight style + nested-language registrations.
//
// The HighlightStyle is Chakra-token driven so the editor follows the
// app theme. It is consumed by `MarkdownEditor` via
// `syntaxHighlighting(markdownHighlightStyle)`.
//
// `dbSqlLanguages` registers `db`, `db-postgres`, `db-mysql`, `db-sqlite`
// as SQL so markdown's nested-code syntax highlighter colorizes the body
// of db fenced blocks. Loaded lazily via dynamic import so the SQL
// language module only ships when at least one DB block exists.

import { LanguageDescription } from "@codemirror/language";
import { HighlightStyle } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { httpTags } from "@httui/lezer-http";

export const dbSqlLanguages: LanguageDescription[] = [
  "db",
  "db-postgres",
  "db-mysql",
  "db-sqlite",
].map((alias) =>
  LanguageDescription.of({
    name: alias,
    alias: [alias],
    async load() {
      const { sql } = await import("@codemirror/lang-sql");
      return sql();
    },
  }),
);

export const markdownHighlightStyle = HighlightStyle.define([
  // Markdown inline formatting
  { tag: tags.strong, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  {
    tag: tags.link,
    color: "var(--chakra-colors-blue-400)",
    textDecoration: "none",
  },
  { tag: tags.url, color: "var(--chakra-colors-blue-400)" },
  {
    tag: tags.monospace,
    fontFamily: "var(--chakra-fonts-mono)",
    fontSize: "0.85em",
  },
  { tag: tags.processingInstruction, color: "var(--chakra-colors-fg-subtle)" },
  { tag: tags.meta, color: "var(--chakra-colors-fg-subtle)" },

  // Code syntax highlighting (for nested languages via codeLanguages)
  { tag: tags.keyword, color: "var(--chakra-colors-purple-500)" },
  {
    tag: [tags.atom, tags.bool, tags.null],
    color: "var(--chakra-colors-orange-500)",
  },
  {
    tag: [tags.number, tags.integer, tags.float],
    color: "var(--chakra-colors-orange-500)",
  },
  {
    tag: [tags.string, tags.special(tags.string)],
    color: "var(--chakra-colors-green-500)",
  },
  { tag: [tags.regexp, tags.escape], color: "var(--chakra-colors-green-400)" },
  {
    tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: "var(--chakra-colors-fg-muted)",
    fontStyle: "italic",
  },
  { tag: [tags.variableName, tags.name], color: "var(--chakra-colors-fg)" },
  {
    tag: [tags.propertyName, tags.attributeName],
    color: "var(--chakra-colors-cyan-400)",
  },
  {
    tag: [tags.typeName, tags.className, tags.namespace],
    color: "var(--chakra-colors-yellow-400)",
  },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
    color: "var(--chakra-colors-blue-400)",
  },
  {
    tag: [
      tags.definition(tags.variableName),
      tags.definition(tags.propertyName),
    ],
    color: "var(--chakra-colors-blue-300)",
  },
  { tag: tags.operator, color: "var(--chakra-colors-pink-400)" },
  {
    tag: [
      tags.punctuation,
      tags.bracket,
      tags.squareBracket,
      tags.paren,
      tags.brace,
    ],
    color: "var(--chakra-colors-fg-subtle)",
  },
  { tag: tags.tagName, color: "var(--chakra-colors-red-400)" },
  {
    tag: tags.self,
    color: "var(--chakra-colors-purple-400)",
    fontStyle: "italic",
  },
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.invalid, color: "var(--chakra-colors-red-500)" },

  // ```http fence body (@httui/lezer-http via codeLanguages). Colors
  // mirror the pre-grammar decoration path: per-method Fuji palette,
  // purple header keys, plain values, dimmed comments/disabled rows.
  { tag: httpTags.method, fontWeight: "600", color: "var(--chakra-colors-fg)" },
  { tag: httpTags.methodGet, color: "var(--chakra-colors-method-get)" },
  { tag: httpTags.methodPost, color: "var(--chakra-colors-method-post)" },
  { tag: httpTags.methodPut, color: "var(--chakra-colors-method-put)" },
  { tag: httpTags.methodPatch, color: "var(--chakra-colors-method-patch)" },
  { tag: httpTags.methodDelete, color: "var(--chakra-colors-method-delete)" },
  { tag: httpTags.methodHead, color: "var(--chakra-colors-method-head)" },
  {
    tag: httpTags.methodOptions,
    color: "var(--chakra-colors-method-options)",
  },
  { tag: httpTags.url, color: "var(--chakra-colors-fg)" },
  {
    tag: httpTags.headerName,
    color: "var(--chakra-colors-purple-500)",
    fontWeight: "500",
  },
  { tag: httpTags.headerValue, color: "var(--chakra-colors-fg)" },
  { tag: httpTags.queryLine, color: "var(--chakra-colors-cyan-600)" },
  {
    tag: httpTags.descLine,
    color: "var(--chakra-colors-teal-500)",
    fontStyle: "italic",
    opacity: "0.85",
  },
  {
    tag: [httpTags.commentLine, httpTags.disabledLine],
    color: "var(--chakra-colors-fg-muted)",
    opacity: "0.7",
  },
]);

// Static CSS for the editor container — @uiw/react-codemirror wraps the
// editor in its own div, which needs explicit height for .cm-scroller
// to work. Extracted as a module-level constant so the Emotion engine
// can cache the serialized rules across re-renders.
export const containerCss = {
  "& > div": { height: "100%" },
  "& .cm-editor": { height: "100%" },
  "& .cm-editor.cm-focused": { outline: "none" },
};
