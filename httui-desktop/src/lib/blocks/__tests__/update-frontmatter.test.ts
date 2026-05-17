import { describe, expect, it } from "vitest";

import {
  updateFrontmatterAbstract,
  updateFrontmatterTasks,
  updateFrontmatterTags,
  updateFrontmatterTitle,
} from "@/lib/blocks/update-frontmatter";

describe("updateFrontmatterTitle", () => {
  it("returns content unchanged when the new title is empty", () => {
    const before = "---\ntitle: existing\n---\nbody\n";
    expect(updateFrontmatterTitle(before, "")).toBe(before);
    expect(updateFrontmatterTitle(before, "   ")).toBe(before);
  });

  it("prepends a new frontmatter when the doc has none", () => {
    const before = "# Heading\n\nbody body\n";
    const after = updateFrontmatterTitle(before, "Hello");
    expect(after).toBe("---\ntitle: Hello\n---\n\n# Heading\n\nbody body\n");
  });

  it("prepends a new frontmatter on an empty document", () => {
    expect(updateFrontmatterTitle("", "Hello")).toBe(
      "---\ntitle: Hello\n---\n\n",
    );
  });

  it("replaces an existing title line in place", () => {
    const before = "---\ntitle: Old\nabstract: keep me\n---\nbody\n";
    const after = updateFrontmatterTitle(before, "New");
    expect(after).toBe("---\ntitle: New\nabstract: keep me\n---\nbody\n");
  });

  it("inserts a title line into a frontmatter that has other keys", () => {
    const before = "---\nabstract: keep me\ntags: [a]\n---\nbody\n";
    const after = updateFrontmatterTitle(before, "Hello");
    expect(after).toBe(
      "---\ntitle: Hello\nabstract: keep me\ntags: [a]\n---\nbody\n",
    );
  });

  it("inserts a title into an empty frontmatter block", () => {
    const before = "---\n---\nbody\n";
    const after = updateFrontmatterTitle(before, "Hello");
    expect(after).toBe("---\ntitle: Hello\n---\nbody\n");
  });

  it("preserves the body across edits (idempotent on body chars)", () => {
    const body = "# Notes\n\nLine 1\n```ts\nconst x = 1;\n```\nLine 2\n";
    const before = "---\ntitle: A\n---\n" + body;
    const after = updateFrontmatterTitle(before, "B");
    expect(after).toBe("---\ntitle: B\n---\n" + body);
  });

  it("quotes titles that contain YAML-special characters", () => {
    expect(updateFrontmatterTitle("", "a:b")).toContain('title: "a:b"');
    expect(updateFrontmatterTitle("", "# heading")).toContain(
      'title: "# heading"',
    );
    // Numeric-looking titles get quoted so the YAML parser doesn't
    // coerce them into integers.
    expect(updateFrontmatterTitle("", "2026")).toContain('title: "2026"');
  });

  it("trims leading/trailing whitespace before writing", () => {
    expect(updateFrontmatterTitle("", "   Hello   ")).toContain("title: Hello");
  });

  it("handles CRLF documents", () => {
    const before = "---\r\ntitle: Old\r\n---\r\nbody\r\n";
    const after = updateFrontmatterTitle(before, "New");
    // Closing fence + body keep their CRLF; only the title line we
    // emit uses LF (round-trip through the editor will normalize).
    expect(after).toContain("title: New");
    expect(after).toContain("---\r\nbody\r\n");
  });

  it("ignores `title:` keys nested under another field (indented line)", () => {
    // Indented `title:` is not a top-level key per the slice-1 schema.
    const before = "---\nmeta:\n  title: nested\n---\nbody\n";
    const after = updateFrontmatterTitle(before, "Top");
    expect(after).toBe("---\ntitle: Top\nmeta:\n  title: nested\n---\nbody\n");
  });

  it("only modifies the first occurrence of `title:`", () => {
    // Malformed input with duplicate keys — first wins, mirrors parser.
    const before = "---\ntitle: First\ntitle: Second\n---\nbody\n";
    const after = updateFrontmatterTitle(before, "Renamed");
    expect(after).toBe("---\ntitle: Renamed\ntitle: Second\n---\nbody\n");
  });
});

describe("updateFrontmatterAbstract", () => {
  it("returns content unchanged when the new abstract is empty", () => {
    const before = "---\ntitle: x\nabstract: keep\n---\nbody\n";
    expect(updateFrontmatterAbstract(before, "")).toBe(before);
    expect(updateFrontmatterAbstract(before, "   \n  ")).toBe(before);
  });

  it("prepends a new frontmatter when the doc has none", () => {
    const before = "# heading\nbody\n";
    expect(updateFrontmatterAbstract(before, "Cool note")).toBe(
      "---\nabstract: Cool note\n---\n\n# heading\nbody\n",
    );
  });

  it("inserts after the title when the field is missing", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    expect(updateFrontmatterAbstract(before, "Cool note")).toBe(
      "---\ntitle: x\nabstract: Cool note\n---\nbody\n",
    );
  });

  it("replaces an existing abstract line in place", () => {
    const before = "---\ntitle: x\nabstract: old\n---\nbody\n";
    expect(updateFrontmatterAbstract(before, "new")).toBe(
      "---\ntitle: x\nabstract: new\n---\nbody\n",
    );
  });

  it("collapses multi-line input to a single space", () => {
    const before = "";
    const after = updateFrontmatterAbstract(
      before,
      "line one\nline two\n  line three",
    );
    // No double-spaces from the original \n + indent.
    expect(after).toContain("abstract: line one line two line three");
  });

  it("inserts at the end of an existing frontmatter without a title", () => {
    const before = "---\ntags: [a]\n---\nbody\n";
    expect(updateFrontmatterAbstract(before, "info")).toBe(
      "---\ntags: [a]\nabstract: info\n---\nbody\n",
    );
  });

  it("quotes special characters", () => {
    expect(updateFrontmatterAbstract("", "hello: world")).toContain(
      'abstract: "hello: world"',
    );
  });
});

describe("updateFrontmatterTags", () => {
  it("returns content unchanged when removing tags from a doc that has none", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    expect(updateFrontmatterTags(before, [])).toBe(before);
  });

  it("returns content unchanged when removing all tags from a doc with no frontmatter", () => {
    const before = "# heading\nbody\n";
    expect(updateFrontmatterTags(before, [])).toBe(before);
  });

  it("removes the tags line when the new list is empty", () => {
    const before = "---\ntitle: x\ntags: [a, b]\n---\nbody\n";
    expect(updateFrontmatterTags(before, [])).toBe(
      "---\ntitle: x\n---\nbody\n",
    );
  });

  it("prepends a fresh frontmatter when adding tags to a doc with none", () => {
    expect(updateFrontmatterTags("body\n", ["alpha", "beta"])).toBe(
      "---\ntags: [alpha, beta]\n---\n\nbody\n",
    );
  });

  it("inserts at the end of an existing frontmatter when missing", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    expect(updateFrontmatterTags(before, ["a", "b"])).toBe(
      "---\ntitle: x\ntags: [a, b]\n---\nbody\n",
    );
  });

  it("replaces an existing tags line in place", () => {
    const before = "---\ntitle: x\ntags: [a, b]\n---\nbody\n";
    expect(updateFrontmatterTags(before, ["c", "d"])).toBe(
      "---\ntitle: x\ntags: [c, d]\n---\nbody\n",
    );
  });

  it("dedupes (first wins) and trims entries", () => {
    expect(
      updateFrontmatterTags("body\n", ["  alpha", "alpha", "beta  "]),
    ).toBe("---\ntags: [alpha, beta]\n---\n\nbody\n");
  });

  it("filters out empty / whitespace-only entries", () => {
    expect(updateFrontmatterTags("body\n", ["alpha", "", "  ", "beta"])).toBe(
      "---\ntags: [alpha, beta]\n---\n\nbody\n",
    );
  });

  it("quotes entries that contain commas or brackets", () => {
    const out = updateFrontmatterTags("body\n", ["a,b", "[c]", "ok"]);
    expect(out).toContain('tags: ["a,b", "[c]", ok]');
  });

  it("preserves the body across edits", () => {
    const body = "# Notes\n\nLine 1\n```ts\nconst x = 1;\n```\nLine 2\n";
    const before = "---\ntags: [a]\n---\n" + body;
    expect(updateFrontmatterTags(before, ["b"])).toBe(
      "---\ntags: [b]\n---\n" + body,
    );
  });
});

describe("updateFrontmatterTasks", () => {
  it("inserts a fresh frontmatter when adding items to a doc with none", () => {
    expect(
      updateFrontmatterTasks("body\n", [{ text: "First", done: false }]),
    ).toBe('---\ntasks: ["[ ] First"]\n---\n\nbody\n');
  });

  it("returns content unchanged when removing items from a doc that has none", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    expect(updateFrontmatterTasks(before, [])).toBe(before);
  });

  it("removes the line when the new list is empty", () => {
    const before = '---\ntitle: x\ntasks: ["[ ] foo"]\n---\nbody\n';
    expect(updateFrontmatterTasks(before, [])).toBe(
      "---\ntitle: x\n---\nbody\n",
    );
  });

  it("inserts at the end of an existing frontmatter when missing", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    expect(
      updateFrontmatterTasks(before, [
        { text: "Verify", done: false },
        { text: "Done item", done: true },
      ]),
    ).toBe(
      '---\ntitle: x\ntasks: ["[ ] Verify", "[x] Done item"]\n---\nbody\n',
    );
  });

  it("replaces an existing tasks line in place", () => {
    const before = '---\ntasks: ["[ ] old"]\n---\nbody\n';
    expect(updateFrontmatterTasks(before, [{ text: "new", done: true }])).toBe(
      '---\ntasks: ["[x] new"]\n---\nbody\n',
    );
  });
});
