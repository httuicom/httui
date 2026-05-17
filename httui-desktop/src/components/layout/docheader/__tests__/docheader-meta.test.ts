import { describe, expect, it } from "vitest";

import {
  authorInitialsFromFirstCommit,
  formatBranchSummary,
  formatEditedTime,
  formatLastRun,
  lastRunTone,
  type LastRunSummary,
} from "../docheader-meta";

describe("authorInitialsFromFirstCommit", () => {
  it("returns first + last initials for two-word names", () => {
    expect(
      authorInitialsFromFirstCommit({
        name: "Jane Doe",
        email: null,
      }),
    ).toBe("JD");
  });

  it("returns first two letters for single-word names", () => {
    expect(authorInitialsFromFirstCommit({ name: "alice", email: null })).toBe(
      "AL",
    );
  });

  it("falls back to email local-part when name is empty", () => {
    expect(
      authorInitialsFromFirstCommit({ name: "", email: "joao@x.test" }),
    ).toBe("JO");
  });

  it("returns ? when name and email are both empty", () => {
    expect(authorInitialsFromFirstCommit({ name: "", email: "" })).toBe("?");
    expect(authorInitialsFromFirstCommit({ name: null, email: null })).toBe(
      "?",
    );
  });
});

describe("formatEditedTime", () => {
  const now = Date.parse("2026-05-02T13:00:00Z");

  it("reports 'just now' under a minute", () => {
    expect(formatEditedTime(now - 30_000, false, now)).toBe("Edited just now");
  });

  it("reports minutes when under an hour", () => {
    expect(formatEditedTime(now - 5 * 60_000, false, now)).toBe(
      "Edited 5m ago",
    );
  });

  it("reports hours when under a day", () => {
    expect(formatEditedTime(now - 3 * 3600_000, false, now)).toBe(
      "Edited 3h ago",
    );
  });

  it("reports days otherwise", () => {
    expect(formatEditedTime(now - 2 * 86400_000, false, now)).toBe(
      "Edited 2d ago",
    );
  });

  it("appends · unsaved when dirty", () => {
    expect(formatEditedTime(now - 30_000, true, now)).toBe(
      "Edited just now · unsaved",
    );
  });

  it("returns 'Not yet saved' when mtime is null and not dirty", () => {
    expect(formatEditedTime(null, false, now)).toBe("Not yet saved");
  });

  it("returns 'Edited just now' when mtime is null and dirty", () => {
    expect(formatEditedTime(null, true, now)).toBe("Edited just now");
  });
});

describe("formatBranchSummary", () => {
  it("includes branch name + +N + ~M when both non-zero", () => {
    expect(
      formatBranchSummary({
        branch: "main",
        addedLines: 12,
        modifiedLines: 4,
      }),
    ).toBe("Branch main +12 ~4");
  });

  it("omits +N when added is 0", () => {
    expect(
      formatBranchSummary({
        branch: "main",
        addedLines: 0,
        modifiedLines: 4,
      }),
    ).toBe("Branch main ~4");
  });

  it("omits ~M when modified is 0", () => {
    expect(
      formatBranchSummary({
        branch: "main",
        addedLines: 12,
        modifiedLines: 0,
      }),
    ).toBe("Branch main +12");
  });

  it("renders just the branch name when both are 0", () => {
    expect(
      formatBranchSummary({
        branch: "main",
        addedLines: 0,
        modifiedLines: 0,
      }),
    ).toBe("Branch main");
  });

  it("renders (detached) when branch is null", () => {
    expect(
      formatBranchSummary({
        branch: null,
        addedLines: 0,
        modifiedLines: 0,
      }),
    ).toBe("Branch (detached)");
  });
});

describe("formatLastRun", () => {
  function summary(over: Partial<LastRunSummary> = {}): LastRunSummary {
    return {
      ranAt: "2026-05-02T14:32:00Z",
      blockCount: 12,
      failedCount: 1,
      ...over,
    };
  }

  it("renders the canvas-spec'd 'last run HH:MM · N blocks · M failed' shape", () => {
    expect(formatLastRun(summary())).toMatch(
      /^Last run \d{2}:\d{2} · 12 blocks · 1 failed$/,
    );
  });

  it("omits the failed segment when failedCount is 0", () => {
    expect(formatLastRun(summary({ failedCount: 0 }))).toMatch(
      /^Last run \d{2}:\d{2} · 12 blocks$/,
    );
  });

  it("agrees sing/plural on blocks", () => {
    expect(formatLastRun(summary({ blockCount: 1, failedCount: 0 }))).toMatch(
      /^Last run \d{2}:\d{2} · 1 block$/,
    );
  });

  it("returns 'No runs yet' when ranAt is null", () => {
    expect(formatLastRun(summary({ ranAt: null }))).toBe("No runs yet");
  });

  it("returns 'No runs yet' when blockCount is 0 even if ranAt set", () => {
    expect(formatLastRun(summary({ blockCount: 0 }))).toBe("No runs yet");
  });
});

describe("lastRunTone", () => {
  it("returns muted when no runs", () => {
    expect(lastRunTone({ ranAt: null, blockCount: 0, failedCount: 0 })).toBe(
      "muted",
    );
  });

  it("returns ok when all blocks passed", () => {
    expect(
      lastRunTone({
        ranAt: "2026-05-02T14:32:00Z",
        blockCount: 5,
        failedCount: 0,
      }),
    ).toBe("ok");
  });

  it("returns fail when any block failed", () => {
    expect(
      lastRunTone({
        ranAt: "2026-05-02T14:32:00Z",
        blockCount: 5,
        failedCount: 1,
      }),
    ).toBe("fail");
  });
});
