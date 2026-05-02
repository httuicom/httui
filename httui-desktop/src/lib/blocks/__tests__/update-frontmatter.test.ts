import { describe, expect, it } from "vitest";

import { updateFrontmatterTitle } from "@/lib/blocks/update-frontmatter";

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
    expect(updateFrontmatterTitle("", "   Hello   ")).toContain(
      "title: Hello",
    );
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
    expect(after).toBe(
      "---\ntitle: Top\nmeta:\n  title: nested\n---\nbody\n",
    );
  });

  it("only modifies the first occurrence of `title:`", () => {
    // Malformed input with duplicate keys — first wins, mirrors parser.
    const before = "---\ntitle: First\ntitle: Second\n---\nbody\n";
    const after = updateFrontmatterTitle(before, "Renamed");
    expect(after).toBe(
      "---\ntitle: Renamed\ntitle: Second\n---\nbody\n",
    );
  });
});
