import { describe, expect, it } from "vitest";

import { extractOutline } from "@/lib/blocks/outline";

describe("extractOutline", () => {
  it("returns empty for empty input", () => {
    expect(extractOutline("")).toEqual([]);
  });

  it("returns empty when document has no headings", () => {
    expect(extractOutline("just a paragraph\nmore text\n")).toEqual([]);
  });

  it("captures a single H1", () => {
    const r = extractOutline("# Hello world\n");
    expect(r).toEqual([{ level: 1, text: "Hello world", line: 1, offset: 0 }]);
  });

  it("captures H1 / H2 / H3 by default and rejects H4+", () => {
    const r = extractOutline("# A\n## B\n### C\n#### D\n##### E\n###### F\n");
    expect(r.map((e) => e.level)).toEqual([1, 2, 3]);
    expect(r.map((e) => e.text)).toEqual(["A", "B", "C"]);
  });

  it("respects maxLevel option", () => {
    const r = extractOutline("# A\n## B\n### C\n", { maxLevel: 2 });
    expect(r.map((e) => e.level)).toEqual([1, 2]);
  });

  it("strips closing-# atx-style heading markers", () => {
    const r = extractOutline("## Section ##\n");
    expect(r[0].text).toBe("Section");
  });

  it("trims trailing whitespace from heading text", () => {
    const r = extractOutline("# Title  \n");
    expect(r[0].text).toBe("Title");
  });

  it("skips headings inside fenced code blocks (```)", () => {
    const r = extractOutline("# Real\n```http\n# fake\n```\n# Real 2\n");
    expect(r.map((e) => e.text)).toEqual(["Real", "Real 2"]);
  });

  it("skips headings inside fenced code blocks (~~~)", () => {
    const r = extractOutline("# A\n~~~\n## fake\n~~~\n# B\n");
    expect(r.map((e) => e.text)).toEqual(["A", "B"]);
  });

  it("only matches a fence-close with the same marker", () => {
    const r = extractOutline(
      "```\n# fake-inside-block\n~~~ bogus\n```\n# real\n",
    );
    expect(r.map((e) => e.text)).toEqual(["real"]);
  });

  it("does not match `#tag` body patterns (no space after #)", () => {
    const r = extractOutline("#payments and other tags\n");
    expect(r).toEqual([]);
  });

  it("does not match `#` alone", () => {
    const r = extractOutline("#\n# Real\n");
    expect(r.map((e) => e.text)).toEqual(["Real"]);
  });

  it("captures line + offset for navigation", () => {
    const content = "para\n\n# H1\nbody\n## H2\n";
    const r = extractOutline(content);
    expect(r[0].line).toBe(3);
    expect(content.slice(r[0].offset).startsWith("# H1")).toBe(true);
    expect(r[1].line).toBe(5);
    expect(content.slice(r[1].offset).startsWith("## H2")).toBe(true);
  });

  it("skips YAML frontmatter at offset 0", () => {
    const content = '---\ntitle: "X"\nstatus: draft\n---\n# Heading\n';
    const r = extractOutline(content);
    expect(r).toHaveLength(1);
    expect(r[0].text).toBe("Heading");
    expect(r[0].line).toBe(5);
  });

  it("does not treat `---` mid-document as frontmatter close", () => {
    const content = "# A\n---\nsome line\n---\n## B\n";
    const r = extractOutline(content);
    expect(r.map((e) => e.text)).toEqual(["A", "B"]);
  });

  it("handles CRLF line endings", () => {
    const content = "# A\r\n## B\r\n";
    const r = extractOutline(content);
    expect(r.map((e) => e.text)).toEqual(["A", "B"]);
  });

  it("treats unterminated frontmatter as plain body", () => {
    // Open `---` but no closing — fall back to walking from
    // offset 0; the leading `---` line won't match a heading
    // and the rest of the doc gets scanned normally.
    const content = "---\nbroken\n# would-be-heading\n";
    const r = extractOutline(content);
    expect(r).toEqual([
      { level: 1, text: "would-be-heading", line: 3, offset: 11 },
    ]);
  });

  it("handles 20+ blocks for nav-acceptance criterion", () => {
    const lines: string[] = [];
    for (let i = 1; i <= 25; i += 1) {
      lines.push(`# Section ${i}`);
      lines.push(`Block ${i} body line.`);
    }
    const r = extractOutline(lines.join("\n") + "\n");
    expect(r).toHaveLength(25);
    expect(r[0].line).toBe(1);
    expect(r[24].text).toBe("Section 25");
  });

  it("treats trailing newline absence the same as presence", () => {
    expect(extractOutline("# X")).toEqual(extractOutline("# X\n"));
  });
});
