import { describe, expect, it } from "vitest";

import {
  extractFrontmatter,
  extractFrontmatterArchived,
  extractFrontmatterTags,
} from "@/lib/blocks/extract-frontmatter-tags";

describe("extractFrontmatterTags", () => {
  it("returns [] when there's no frontmatter", () => {
    expect(extractFrontmatterTags("# Heading\n\nbody\n")).toEqual([]);
    expect(extractFrontmatterTags("")).toEqual([]);
  });

  it("returns [] when the document has no closing fence", () => {
    expect(extractFrontmatterTags("---\ntags: [a]\nno close\n")).toEqual([]);
  });

  it("extracts tags from a flow-style list", () => {
    const doc = "---\ntitle: foo\ntags: [payments, debug]\n---\nbody\n";
    expect(extractFrontmatterTags(doc)).toEqual(["payments", "debug"]);
  });

  it("unquotes single-quoted entries", () => {
    const doc = "---\ntags: ['hello world', debug]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual(["hello world", "debug"]);
  });

  it("unquotes double-quoted entries", () => {
    const doc = '---\ntags: ["hello world", "debug"]\n---\n';
    expect(extractFrontmatterTags(doc)).toEqual(["hello world", "debug"]);
  });

  it("filters empty entries from `[a, , b]`", () => {
    expect(extractFrontmatterTags("---\ntags: [a, , b]\n---\n")).toEqual([
      "a",
      "b",
    ]);
  });

  it("dedups same-tag-twice within the file", () => {
    expect(extractFrontmatterTags("---\ntags: [foo, foo, bar]\n---\n")).toEqual(
      ["foo", "bar"],
    );
  });

  it("returns [] on block-list shape (out of slice-1 schema)", () => {
    // Drift contract: when the Rust parser learns the block-list
    // shape, this helper must too. Until then, both return empty.
    const doc = "---\ntags:\n  - a\n  - b\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual([]);
  });

  it("returns [] when frontmatter has no `tags:` key", () => {
    expect(
      extractFrontmatterTags("---\ntitle: foo\nowner: alice\n---\n"),
    ).toEqual([]);
  });

  it("ignores indented lines at top level", () => {
    // Indented children belong to a parent block scalar/list — not
    // a top-level `tags:` value.
    const doc = "---\nabstract: |\n  tags: [should-not-pick]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual([]);
  });

  it("ignores comment lines and blank lines", () => {
    const doc =
      "---\n# leading comment\n\ntags: [a, b]\n# trailing comment\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual(["a", "b"]);
  });

  it("handles CRLF line endings", () => {
    const doc = "---\r\ntags: [a, b]\r\n---\r\nbody\r\n";
    expect(extractFrontmatterTags(doc)).toEqual(["a", "b"]);
  });

  it("strips a UTF-8 BOM before the fence", () => {
    const doc = "\u{feff}---\ntags: [a]\n---\nbody\n";
    expect(extractFrontmatterTags(doc)).toEqual(["a"]);
  });

  it("returns [] when the value isn't a flow array", () => {
    expect(extractFrontmatterTags("---\ntags: notabracket\n---\n")).toEqual([]);
    expect(extractFrontmatterTags('---\ntags: "hello"\n---\n')).toEqual([]);
    expect(extractFrontmatterTags("---\ntags: 42\n---\n")).toEqual([]);
  });

  it("returns [] on an empty flow array", () => {
    expect(extractFrontmatterTags("---\ntags: []\n---\n")).toEqual([]);
  });

  it("trims whitespace inside flow entries", () => {
    expect(extractFrontmatterTags("---\ntags: [   a  ,  b ]\n---\n")).toEqual([
      "a",
      "b",
    ]);
  });

  it("skips a line without a colon", () => {
    // Defensive — defended by `colonIdx < 0` continue. Build a
    // synthetic doc with a malformed line right above the tags
    // line.
    const doc = "---\nbroken-line-no-colon\ntags: [ok]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual(["ok"]);
  });

  it("does not match a key whose name only contains 'tags' as a prefix/suffix", () => {
    // `subtags` should not be picked up as `tags`.
    const doc = "---\nsubtags: [ignore]\nstaging: [skip]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual([]);
  });

  it("handles a value without quotes containing colons", () => {
    // The first `:` after the key is the delimiter; later colons in
    // the value are part of the value — but a flow-list shape
    // shouldn't contain bare colons in unquoted entries. This test
    // documents the failure mode (treated as a plain string entry).
    const doc = "---\ntags: [a:b, c]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual(["a:b", "c"]);
  });

  it("works with single 1-tag flow list", () => {
    expect(extractFrontmatterTags("---\ntags: [solo]\n---\n")).toEqual([
      "solo",
    ]);
  });

  it("returns the same array shape for empty body", () => {
    expect(extractFrontmatterTags("---\ntags: [a]\n---\n")).toEqual(["a"]);
  });

  it("ignores subsequent `tags:` lines (first wins)", () => {
    // Defensive: a malformed file with two `tags:` entries —
    // first-wins keeps things deterministic.
    const doc = "---\ntags: [first]\ntags: [second]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual(["first"]);
  });
});

describe("extractFrontmatter", () => {
  it("returns empty shape with tags:[] for content without frontmatter", () => {
    expect(extractFrontmatter("# heading only\n")).toEqual({
      tags: [],
      tasks: [],
    });
    expect(extractFrontmatter("")).toEqual({ tags: [], tasks: [] });
  });

  it("extracts title + abstract + tags from a typical document", () => {
    const doc =
      '---\ntitle: "Payments — debug capture failures"\nabstract: "Capture flow when X"\ntags: [payments, debug]\n---\nbody\n';
    expect(extractFrontmatter(doc)).toEqual({
      title: "Payments — debug capture failures",
      abstract: "Capture flow when X",
      tags: ["payments", "debug"],
      tasks: [],
    });
  });

  it("extracts a tasks checklist", () => {
    const doc = '---\ntitle: x\ntasks: ["[ ] Verify", "[x] Done"]\n---\nbody\n';
    expect(extractFrontmatter(doc).tasks).toEqual([
      { text: "Verify", done: false },
      { text: "Done", done: true },
    ]);
  });

  it("unquotes both quote styles for title", () => {
    expect(extractFrontmatter("---\ntitle: 'single quoted'\n---\n").title).toBe(
      "single quoted",
    );
    expect(extractFrontmatter('---\ntitle: "double quoted"\n---\n').title).toBe(
      "double quoted",
    );
  });

  it("treats `abstract: |` block-scalar marker as undefined (Rust slice-1)", () => {
    // Drift contract: when the Rust parser learns block-scalar
    // bodies, this helper must too. For now the multi-line abstract
    // body is captured by the Rust raw_yaml region only — the TS
    // helper returns abstract = undefined.
    const doc = "---\nabstract: |\n  multi line\n  body here\n---\n";
    expect(extractFrontmatter(doc).abstract).toBeUndefined();
  });

  it("treats `abstract: >` folded-scalar marker as undefined", () => {
    expect(
      extractFrontmatter("---\nabstract: >\n  folded\n---\n").abstract,
    ).toBeUndefined();
  });

  it("returns title undefined when blank / empty-quoted (Rust slice-1)", () => {
    // Mirrors the Rust `parse_typed` behaviour: a literal empty
    // value (after unquote) is dropped. Whitespace-inside-quotes
    // (`'   '`) survives as-is — pickH1Title trims it downstream
    // before falling back through firstHeading → filename.
    expect(extractFrontmatter("---\ntitle: \n---\n").title).toBeUndefined();
    expect(extractFrontmatter('---\ntitle: ""\n---\n').title).toBeUndefined();
    expect(extractFrontmatter("---\ntitle: ''\n---\n").title).toBeUndefined();
  });

  it("first-wins on duplicate `title:` lines", () => {
    expect(
      extractFrontmatter("---\ntitle: First\ntitle: Second\n---\n").title,
    ).toBe("First");
  });

  it("preserves missing optionals as undefined (no shape pollution)", () => {
    const fm = extractFrontmatter("---\ntitle: Solo\n---\n");
    expect(fm.title).toBe("Solo");
    expect(fm.abstract).toBeUndefined();
    expect(fm.tags).toEqual([]);
    expect("abstract" in fm).toBe(false);
  });

  it("ignores indented lines (parent block scalars don't leak)", () => {
    const doc = "---\nabstract: |\n  title: should-not-leak\n---\n";
    expect(extractFrontmatter(doc).title).toBeUndefined();
  });

  it("extractFrontmatterTags wraps extractFrontmatter consistently", () => {
    const doc = "---\ntitle: x\ntags: [a, b]\n---\n";
    expect(extractFrontmatterTags(doc)).toEqual(extractFrontmatter(doc).tags);
  });
});

describe("extractFrontmatter — error path", () => {
  it("doc without a fence has no error", () => {
    expect(extractFrontmatter("# heading\n").error).toBeUndefined();
    expect(extractFrontmatter("").error).toBeUndefined();
  });

  it("flags an unterminated frontmatter block", () => {
    const doc = "---\ntitle: foo\ntags: [a]\nno closing fence\n";
    const fm = extractFrontmatter(doc);
    expect(fm.error).toBeDefined();
    expect(fm.error).toMatch(/não fechado/);
    // Terminal failure: typed values aren't surfaced.
    expect(fm.title).toBeUndefined();
    expect(fm.tags).toEqual([]);
  });

  it("flags `tags:` block-list shape (next line indented + dash)", () => {
    const doc = "---\ntags:\n  - a\n  - b\n---\n";
    const fm = extractFrontmatter(doc);
    expect(fm.error).toBeDefined();
    expect(fm.error).toMatch(/flow-style/);
    expect(fm.tags).toEqual([]);
  });

  it("flags `tasks:` block-list shape", () => {
    const doc = '---\ntasks:\n  - "[ ] do thing"\n---\n';
    expect(extractFrontmatter(doc).error).toBeDefined();
  });

  it("flags `tags:` with a bare scalar value", () => {
    expect(extractFrontmatter("---\ntags: foo\n---\n").error).toBeDefined();
    expect(extractFrontmatter('---\ntags: "x"\n---\n').error).toBeDefined();
    expect(extractFrontmatter("---\ntags: 42\n---\n").error).toBeDefined();
  });

  it("does not flag a typo'd `tags:` followed by another top-level key", () => {
    // Ambiguous user input: empty value with no continuation. Treat as
    // benign — the user might be in the middle of typing.
    const doc = "---\ntags:\ntitle: foo\n---\n";
    expect(extractFrontmatter(doc).error).toBeUndefined();
  });

  it("does not flag an empty flow list `tags: []`", () => {
    expect(extractFrontmatter("---\ntags: []\n---\n").error).toBeUndefined();
  });

  it("does not flag a valid frontmatter", () => {
    const doc =
      '---\ntitle: x\nabstract: y\ntags: [a, b]\ntasks: ["[ ] z"]\n---\nbody\n';
    expect(extractFrontmatter(doc).error).toBeUndefined();
  });

  it("ignores commented continuation lines when probing block-list shape", () => {
    // `tags:` with only a comment beneath it isn't a block-list
    // mistake — the comment is benign and the user has no value yet.
    const doc = "---\ntags:\n  # placeholder\ntitle: foo\n---\n";
    expect(extractFrontmatter(doc).error).toBeUndefined();
  });
});

describe("extractFrontmatter — status", () => {
  it("returns undefined when status: is absent", () => {
    expect(extractFrontmatter("---\ntitle: x\n---\n").status).toBeUndefined();
  });

  it("extracts status: archived", () => {
    expect(extractFrontmatter("---\nstatus: archived\n---\n").status).toBe(
      "archived",
    );
  });

  it("normalizes case and trims whitespace", () => {
    expect(extractFrontmatter("---\nstatus:   ARCHIVED   \n---\n").status).toBe(
      "archived",
    );
  });

  it("preserves forward-compat unknown values (lower-cased)", () => {
    expect(extractFrontmatter("---\nstatus: review\n---\n").status).toBe(
      "review",
    );
  });

  it("unquotes both quote styles", () => {
    expect(extractFrontmatter('---\nstatus: "archived"\n---\n').status).toBe(
      "archived",
    );
    expect(extractFrontmatter("---\nstatus: 'draft'\n---\n").status).toBe(
      "draft",
    );
  });

  it("first-wins on duplicate status: lines", () => {
    expect(
      extractFrontmatter("---\nstatus: draft\nstatus: archived\n---\n").status,
    ).toBe("draft");
  });
});

describe("extractFrontmatterArchived", () => {
  it("true when status: archived", () => {
    expect(extractFrontmatterArchived("---\nstatus: archived\n---\n")).toBe(
      true,
    );
  });

  it("false for any other status (or none)", () => {
    expect(extractFrontmatterArchived("---\nstatus: draft\n---\n")).toBe(false);
    expect(extractFrontmatterArchived("---\ntitle: x\n---\n")).toBe(false);
    expect(extractFrontmatterArchived("# no fence\n")).toBe(false);
  });
});
