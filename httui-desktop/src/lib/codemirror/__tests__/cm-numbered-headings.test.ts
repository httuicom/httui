import { describe, it, expect } from "vitest";
import { Text } from "@codemirror/state";

import { buildHeadingDecorations } from "@/lib/codemirror/cm-numbered-headings";

function asLines(text: string) {
  return Text.of(text.split("\n"));
}

function numbers(decorations: ReturnType<typeof buildHeadingDecorations>) {
  const out: number[] = [];
  decorations.decorations.between(0, Number.MAX_SAFE_INTEGER, (_f, _t, dec) => {
    const num = dec.spec?.attributes?.["data-heading-number"];
    if (num) out.push(Number(num));
  });
  return out;
}

function levels(decorations: ReturnType<typeof buildHeadingDecorations>) {
  const out: number[] = [];
  decorations.decorations.between(0, Number.MAX_SAFE_INTEGER, (_f, _t, dec) => {
    const level = dec.spec?.attributes?.["data-heading-level"];
    if (level) out.push(Number(level));
  });
  return out;
}

describe("buildHeadingDecorations", () => {
  it("numbers a flat list of h1+h2 sequentially", () => {
    const doc = asLines("# A\n\n## B\n\n# C\n");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(3);
    expect(numbers(result)).toEqual([1, 2, 3]);
  });

  it("counts only top-level # and ## (skips ### h3 and deeper)", () => {
    const doc = asLines("# A\n## B\n### should-skip\n#### also-skip\n# C\n");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(3);
  });

  it("skips headings inside fenced code blocks", () => {
    const doc = asLines(
      "# A\n\n```\n# inside-fence\n## inside-fence-2\n```\n\n# B\n",
    );
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(2);
    expect(numbers(result)).toEqual([1, 2]);
  });

  it("recognises ~~~ fences too", () => {
    const doc = asLines("# A\n\n~~~\n# inside\n~~~\n\n# B\n");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(2);
  });

  it("ignores non-heading lines that begin with #", () => {
    // `#tag` (no space) is not a heading per CommonMark.
    const doc = asLines("#tag\n# real heading\n");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(1);
  });

  it("returns no decorations for an empty doc", () => {
    const doc = asLines("");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(0);
  });

  it("ignores headings with no following text (just `#`)", () => {
    const doc = asLines("#\n## \n# real\n");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(1);
  });

  it("attributes carry the positional number, starting at 1", () => {
    const doc = asLines("# first\n# second\n# third\n");
    const result = buildHeadingDecorations(doc);
    expect(numbers(result)).toEqual([1, 2, 3]);
  });

  it("nested fence with mismatched closer keeps tracking", () => {
    // Open ```, then `~~~` (wrong marker — counts as content), then
    // closing ```. Headings between the two ``` are still skipped.
    const doc = asLines("# A\n```\n~~~\n# inside\n~~~\n```\n# B\n");
    const result = buildHeadingDecorations(doc);
    expect(result.count).toBe(2);
  });

  it("emits data-heading-level matching the marker length", () => {
    // `#` → level 1, `##` → level 2. Level powers the H1-only
    // typography rule in editor-theme.
    const doc = asLines("# H1\n## H2\n# H1 again\n## H2 again\n");
    const result = buildHeadingDecorations(doc);
    expect(levels(result)).toEqual([1, 2, 1, 2]);
  });
});
