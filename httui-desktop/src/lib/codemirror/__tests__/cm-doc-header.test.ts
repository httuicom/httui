import { describe, expect, it } from "vitest";
import { Text } from "@codemirror/state";

import { findFrontmatterRange } from "@/lib/codemirror/cm-doc-header";

function asDoc(text: string) {
  return Text.of(text.split("\n"));
}

describe("findFrontmatterRange", () => {
  it("returns null for an empty doc", () => {
    expect(findFrontmatterRange(asDoc(""))).toBeNull();
  });

  it("returns null when the doc has no opening fence", () => {
    expect(findFrontmatterRange(asDoc("# Heading\n\nbody"))).toBeNull();
  });

  it("returns null when the opening fence is not on line 1", () => {
    expect(findFrontmatterRange(asDoc("\n---\ntitle: x\n---\n"))).toBeNull();
  });

  it("returns null when the opening fence has no matching close", () => {
    expect(
      findFrontmatterRange(asDoc("---\ntitle: x\nbody body body")),
    ).toBeNull();
  });

  it("detects a simple single-key frontmatter", () => {
    const doc = asDoc("---\ntitle: Hello\n---\nbody");
    const range = findFrontmatterRange(doc);
    // 0..3 = "---", 4..16 = "title: Hello\n", 17..19 = "---"
    // After the closing `---` line.to == 20 (end of line 3 inclusive of \n).
    // We swallow the trailing newline so body cursor lands on offset 20.
    expect(range).not.toBeNull();
    expect(range!.from).toBe(0);
    // Range covers `---\ntitle: Hello\n---\n` = 21 chars.
    expect(range!.to).toBe(21);
  });

  it("detects a multi-line frontmatter", () => {
    const doc = asDoc(
      "---\ntitle: Hello\nabstract: World\ntags: [a, b]\n---\nbody",
    );
    const range = findFrontmatterRange(doc);
    expect(range).not.toBeNull();
    expect(range!.from).toBe(0);
    // Length of frontmatter incl. trailing \n: 51 ?
    // Let's compute: "---\n" = 4, "title: Hello\n" = 13, "abstract: World\n" = 16,
    // "tags: [a, b]\n" = 13, "---\n" = 4. Total = 50.
    expect(range!.to).toBe(50);
  });

  it("does not confuse a `---` separator in the middle of the body", () => {
    const doc = asDoc("# Heading\n\n---\n\nbelow the rule");
    expect(findFrontmatterRange(doc)).toBeNull();
  });

  it("requires the close fence to be exactly `---`", () => {
    // `--- ` with trailing space is not a fence terminator (must be exact).
    const doc = asDoc("---\ntitle: x\n--- \nbody");
    expect(findFrontmatterRange(doc)).toBeNull();
  });

  it("handles frontmatter that occupies the entire doc (no body)", () => {
    const doc = asDoc("---\ntitle: x\n---");
    const range = findFrontmatterRange(doc);
    expect(range).not.toBeNull();
    expect(range!.from).toBe(0);
    // No trailing newline to swallow — `to` is doc length.
    expect(range!.to).toBe(doc.length);
  });

  it("handles an empty frontmatter body", () => {
    const doc = asDoc("---\n---\n# body");
    const range = findFrontmatterRange(doc);
    expect(range).not.toBeNull();
    // "---\n---\n" = 8 chars.
    expect(range!.to).toBe(8);
  });
});
