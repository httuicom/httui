import { describe, expect, it } from "vitest";

import { pluralizeFiles, validateCommitMessage } from "../git-commit-validate";

describe("validateCommitMessage", () => {
  it("rejects an empty message", () => {
    const v = validateCommitMessage("");
    expect(v.valid).toBe(false);
    expect(v.errors[0]).toMatch(/empty/i);
    expect(v.subject).toBe("");
    expect(v.body).toBe("");
  });

  it("rejects a whitespace-only message", () => {
    const v = validateCommitMessage("   \n  \n");
    expect(v.valid).toBe(false);
    expect(v.errors[0]).toMatch(/empty/i);
  });

  it("accepts a single-line subject", () => {
    const v = validateCommitMessage("fix bug");
    expect(v.valid).toBe(true);
    expect(v.subject).toBe("fix bug");
    expect(v.body).toBe("");
  });

  it("splits subject and body on the first blank line", () => {
    const v = validateCommitMessage(
      "subject line\n\nbody paragraph 1\n\nbody paragraph 2",
    );
    expect(v.valid).toBe(true);
    expect(v.subject).toBe("subject line");
    expect(v.body).toBe("body paragraph 1\n\nbody paragraph 2");
  });

  it("flags an oversized subject (>72 chars)", () => {
    const long = "a".repeat(73);
    const v = validateCommitMessage(long);
    expect(v.valid).toBe(false);
    expect(v.errors.some((e) => /under 72/i.test(e))).toBe(true);
  });

  it("flags leading whitespace on the subject", () => {
    const v = validateCommitMessage("  spaced subject");
    expect(v.valid).toBe(false);
    expect(v.errors.some((e) => /leading whitespace/i.test(e))).toBe(true);
  });

  it("trims trailing whitespace before processing", () => {
    const v = validateCommitMessage("subject   \n");
    expect(v.valid).toBe(true);
    expect(v.subject).toBe("subject");
  });

  it("treats a message with body but no blank-line separator as subject only", () => {
    const v = validateCommitMessage("line1\nline2");
    expect(v.body).toBe("");
    expect(v.subject).toBe("line1");
  });
});

describe("pluralizeFiles", () => {
  it("agrees singular when 1", () => {
    expect(pluralizeFiles(1)).toBe("1 file");
  });

  it("agrees plural for 0 and 2+", () => {
    expect(pluralizeFiles(0)).toBe("0 files");
    expect(pluralizeFiles(2)).toBe("2 files");
    expect(pluralizeFiles(99)).toBe("99 files");
  });
});
