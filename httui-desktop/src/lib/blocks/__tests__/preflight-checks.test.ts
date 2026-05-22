import { describe, expect, it } from "vitest";

import {
  extractPreflightChecks,
  updateFrontmatterPreflightChecks,
  type PreflightCheck,
} from "@/lib/blocks/preflight-checks";

describe("extractPreflightChecks", () => {
  it("returns [] when there's no frontmatter", () => {
    expect(extractPreflightChecks("# heading\n")).toEqual([]);
    expect(extractPreflightChecks("")).toEqual([]);
  });

  it("returns [] when frontmatter has no preflight section", () => {
    const doc = "---\ntitle: x\ntags: [a]\n---\nbody\n";
    expect(extractPreflightChecks(doc)).toEqual([]);
  });

  it("parses every supported kind", () => {
    const doc = [
      "---",
      "preflight:",
      "  - connection: payments-db",
      "  - env_var: API_TOKEN",
      "  - branch: main",
      "  - file_exists: ./schema/payments.sql",
      "  - command: psql --version",
      "---",
      "body",
      "",
    ].join("\n");
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "connection", value: "payments-db" },
      { kind: "env_var", value: "API_TOKEN" },
      { kind: "branch", value: "main" },
      { kind: "file_exists", value: "./schema/payments.sql" },
      { kind: "command", value: "psql --version" },
    ]);
  });

  it("drops retired keychain entries as unknown kinds", () => {
    const doc =
      "---\npreflight:\n  - keychain: payments-db.password\n  - connection: ok\n---\n";
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "connection", value: "ok" },
    ]);
  });

  it("unquotes values with quotes", () => {
    const doc =
      "---\npreflight:\n  - file_exists: \"./creds/api.json\"\n  - command: 'psql -U admin'\n---\n";
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "file_exists", value: "./creds/api.json" },
      { kind: "command", value: "psql -U admin" },
    ]);
  });

  it("ignores unknown kinds (forward-compat fallback)", () => {
    const doc = "---\npreflight:\n  - future_kind: anything\n---\n";
    expect(extractPreflightChecks(doc)).toEqual([]);
  });

  it("ignores items without a colon", () => {
    const doc = "---\npreflight:\n  - just-a-note\n---\n";
    expect(extractPreflightChecks(doc)).toEqual([]);
  });

  it("stops at the next top-level key", () => {
    const doc =
      "---\npreflight:\n  - connection: a\ntitle: x\n  - command: ls\n---\n";
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "connection", value: "a" },
    ]);
  });

  it("tolerates blank lines inside the block", () => {
    const doc = "---\npreflight:\n  - connection: a\n\n  - env_var: B\n---\n";
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "connection", value: "a" },
      { kind: "env_var", value: "B" },
    ]);
  });

  it("tolerates tab indentation", () => {
    const doc = "---\npreflight:\n\t- command: ls\n---\n";
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "command", value: "ls" },
    ]);
  });

  it("first preflight: header wins over a duplicate", () => {
    const doc =
      "---\npreflight:\n  - connection: first\npreflight:\n  - connection: second\n---\n";
    expect(extractPreflightChecks(doc)).toEqual<PreflightCheck[]>([
      { kind: "connection", value: "first" },
    ]);
  });
});

describe("updateFrontmatterPreflightChecks", () => {
  it("inserts a fresh frontmatter when the doc has none", () => {
    const next = updateFrontmatterPreflightChecks("body\n", [
      { kind: "connection", value: "payments-db" },
    ]);
    expect(next).toBe(
      "---\npreflight:\n  - connection: payments-db\n---\n\nbody\n",
    );
  });

  it("inserts at the end of an existing frontmatter when missing", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    const next = updateFrontmatterPreflightChecks(before, [
      { kind: "command", value: "psql" },
    ]);
    expect(next).toBe(
      "---\ntitle: x\npreflight:\n  - command: psql\n---\nbody\n",
    );
  });

  it("replaces an existing block in place", () => {
    const before = "---\npreflight:\n  - connection: old\n---\nbody\n";
    const next = updateFrontmatterPreflightChecks(before, [
      { kind: "connection", value: "new" },
      { kind: "env_var", value: "X" },
    ]);
    expect(next).toBe(
      "---\npreflight:\n  - connection: new\n  - env_var: X\n---\nbody\n",
    );
  });

  it("removes the block when checks is empty", () => {
    const before =
      "---\ntitle: x\npreflight:\n  - connection: a\n  - env_var: B\n---\nbody\n";
    const next = updateFrontmatterPreflightChecks(before, []);
    expect(next).toBe("---\ntitle: x\n---\nbody\n");
  });

  it("returns content unchanged when removing from a doc that has no block", () => {
    const before = "---\ntitle: x\n---\nbody\n";
    expect(updateFrontmatterPreflightChecks(before, [])).toBe(before);
  });

  it("emits values with internal spaces unquoted (block-list YAML allows it)", () => {
    const next = updateFrontmatterPreflightChecks("---\n---\n", [
      { kind: "command", value: "psql -U admin" },
    ]);
    expect(next).toContain("- command: psql -U admin");
  });

  it("quotes values that start with a YAML scalar marker", () => {
    const next = updateFrontmatterPreflightChecks("---\n---\n", [
      { kind: "command", value: "-U admin" },
    ]);
    expect(next).toContain('- command: "-U admin"');
  });

  it("quotes values containing a colon (would otherwise re-parse as nested key)", () => {
    const next = updateFrontmatterPreflightChecks("---\n---\n", [
      { kind: "file_exists", value: "C:/win/path" },
    ]);
    expect(next).toContain('- file_exists: "C:/win/path"');
  });

  it("quotes empty values defensively", () => {
    const next = updateFrontmatterPreflightChecks("---\n---\n", [
      { kind: "command", value: "" },
    ]);
    expect(next).toContain('- command: ""');
  });

  it("round-trips: parse → update with same set → unchanged content", () => {
    const before =
      "---\npreflight:\n  - connection: a\n  - command: ls\n---\nbody\n";
    const checks = extractPreflightChecks(before);
    const after = updateFrontmatterPreflightChecks(before, checks);
    expect(after).toBe(before);
  });

  it("preserves non-preflight frontmatter keys", () => {
    const before =
      "---\ntitle: hello\ntags: [a, b]\npreflight:\n  - connection: x\n---\nbody\n";
    const next = updateFrontmatterPreflightChecks(before, [
      { kind: "connection", value: "y" },
    ]);
    expect(next).toContain("title: hello");
    expect(next).toContain("tags: [a, b]");
    expect(next).toContain("connection: y");
  });
});
