import { describe, expect, it } from "vitest";

import { deriveCommitMessage, noteStem } from "@/lib/blocks/commit-template";

describe("noteStem", () => {
  it("strips a trailing .md", () => {
    expect(noteStem("notes/foo.md")).toBe("foo");
    expect(noteStem("foo.md")).toBe("foo");
  });

  it("keeps non-md filenames intact", () => {
    expect(noteStem(".httui/connections.toml")).toBe("connections.toml");
    expect(noteStem("a/b/c")).toBe("c");
  });
});

describe("deriveCommitMessage — default (no template)", () => {
  it("is empty when nothing changed", () => {
    expect(deriveCommitMessage([], null)).toBe("");
  });

  it("names the single changed note", () => {
    expect(deriveCommitMessage(["notes/rollout.md"], null)).toBe(
      "Update rollout",
    );
  });

  it("counts when several notes changed", () => {
    expect(deriveCommitMessage(["a.md", "b.md", "sub/c.md"], undefined)).toBe(
      "Update 3 notes",
    );
  });

  it("treats a blank template as no template", () => {
    expect(deriveCommitMessage(["a.md"], "   ")).toBe("Update a");
  });
});

describe("deriveCommitMessage — configured template", () => {
  const NOW = new Date(2026, 4, 16); // 2026-05-16 local

  it("renders {{notes}} / {{count}} / {{date}}", () => {
    expect(
      deriveCommitMessage(
        ["notes/a.md", "notes/b.md"],
        "docs: {{notes}} ({{count}}) {{date}}",
        NOW,
      ),
    ).toBe("docs: a, b (2) 2026-05-16");
  });

  it("tolerates whitespace inside the placeholders", () => {
    expect(deriveCommitMessage(["x.md"], "{{ count }}-{{ notes }}", NOW)).toBe(
      "1-x",
    );
  });

  it("zero-pads month and day in {{date}}", () => {
    expect(
      deriveCommitMessage(["x.md"], "{{date}}", new Date(2026, 0, 3)),
    ).toBe("2026-01-03");
  });
});
