import { describe, expect, it } from "vitest";

import {
  ABSTRACT_FADE_THRESHOLD,
  deriveAbstractDisplay,
  deriveBreadcrumb,
  filenameWithoutExtension,
  pickH1Title,
  type DocHeaderFrontmatter,
} from "../docheader-derive";

describe("pickH1Title", () => {
  it("prefers frontmatter title when present", () => {
    expect(pickH1Title({ title: "Custom" }, "Heading", "notes/db.md")).toBe(
      "Custom",
    );
  });

  it("treats blank frontmatter title as absent", () => {
    expect(pickH1Title({ title: "   " }, "Heading", "notes/db.md")).toBe(
      "Heading",
    );
  });

  it("falls back to first heading when frontmatter title is missing", () => {
    expect(pickH1Title(null, "First H1", "notes/db.md")).toBe("First H1");
  });

  it("falls back to filename when no frontmatter and no heading", () => {
    expect(pickH1Title(null, null, "notes/db-runbook.md")).toBe("db-runbook");
  });

  it("trims surrounding whitespace on the picked value", () => {
    expect(pickH1Title({ title: "  Hi  " }, null, "x.md")).toBe("Hi");
  });
});

describe("filenameWithoutExtension", () => {
  it("strips the .md extension", () => {
    expect(filenameWithoutExtension("notes/db.md")).toBe("db");
  });

  it("works for paths without a directory component", () => {
    expect(filenameWithoutExtension("scratch.md")).toBe("scratch");
  });

  it("returns the basename when there is no extension", () => {
    expect(filenameWithoutExtension("notes/README")).toBe("README");
  });

  it("preserves leading dots in dotfiles", () => {
    expect(filenameWithoutExtension(".gitignore")).toBe(".gitignore");
  });

  it("handles backslash separators (Windows-pasted paths)", () => {
    expect(filenameWithoutExtension("notes\\db.md")).toBe("db");
  });
});

describe("deriveBreadcrumb", () => {
  it("returns one segment per path component, leaf without extension", () => {
    expect(deriveBreadcrumb("notes/runbooks/db.md")).toEqual([
      { label: "notes", path: "notes" },
      { label: "runbooks", path: "notes/runbooks" },
      { label: "db", path: "notes/runbooks/db.md" },
    ]);
  });

  it("returns a single leaf segment for a flat file", () => {
    expect(deriveBreadcrumb("scratch.md")).toEqual([
      { label: "scratch", path: "scratch.md" },
    ]);
  });

  it("returns an empty array for an empty path", () => {
    expect(deriveBreadcrumb("")).toEqual([]);
  });

  it("strips leading slashes", () => {
    expect(deriveBreadcrumb("/notes/db.md")).toEqual([
      { label: "notes", path: "notes" },
      { label: "db", path: "notes/db.md" },
    ]);
  });

  it("normalizes Windows-style backslashes", () => {
    expect(deriveBreadcrumb("notes\\db.md")).toEqual([
      { label: "notes", path: "notes" },
      { label: "db", path: "notes/db.md" },
    ]);
  });
});

describe("deriveAbstractDisplay", () => {
  it("returns null when frontmatter has no abstract", () => {
    expect(deriveAbstractDisplay(null)).toBeNull();
    expect(deriveAbstractDisplay({})).toBeNull();
    expect(deriveAbstractDisplay({ abstract: "" })).toBeNull();
    expect(deriveAbstractDisplay({ abstract: "   " })).toBeNull();
  });

  it("flags short abstracts as not needing truncation", () => {
    const out = deriveAbstractDisplay({ abstract: "Short summary." });
    expect(out?.needsTruncation).toBe(false);
  });

  it("flags long abstracts (>250 chars) as needing truncation", () => {
    const long: DocHeaderFrontmatter = {
      abstract: "x".repeat(ABSTRACT_FADE_THRESHOLD + 1),
    };
    const out = deriveAbstractDisplay(long);
    expect(out?.needsTruncation).toBe(true);
    expect(out?.text.length).toBeGreaterThan(ABSTRACT_FADE_THRESHOLD);
  });

  it("trims surrounding whitespace from the abstract", () => {
    const out = deriveAbstractDisplay({ abstract: "  Hello  " });
    expect(out?.text).toBe("Hello");
  });
});
