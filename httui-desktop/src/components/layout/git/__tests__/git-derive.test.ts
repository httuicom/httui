import { describe, expect, it } from "vitest";

import type { CommitInfo, GitFileChange, GitStatus } from "@/lib/tauri/git";

import {
  authorInitials,
  labelFileStatus,
  partitionFileChanges,
  relativeTime,
  summarizeBranch,
  summarizeChangeCounts,
} from "../git-derive";

function status(over: Partial<GitStatus> = {}): GitStatus {
  return {
    branch: "main",
    upstream: "origin/main",
    ahead: 0,
    behind: 0,
    changed: [],
    clean: true,
    ...over,
  };
}

function fc(over: Partial<GitFileChange> = {}): GitFileChange {
  return {
    path: "a",
    status: "M.",
    staged: false,
    untracked: false,
    ...over,
  };
}

function commit(over: Partial<CommitInfo> = {}): CommitInfo {
  return {
    sha: "deadbeef0000",
    short_sha: "deadbee",
    author_name: "Jane Doe",
    author_email: "jane@x.test",
    timestamp: Math.floor(Date.now() / 1000) - 30,
    subject: "first commit",
    ...over,
  };
}

describe("partitionFileChanges", () => {
  it("groups by staged / unstaged / untracked", () => {
    const out = partitionFileChanges([
      fc({ path: "a", staged: true }),
      fc({ path: "b", staged: false }),
      fc({ path: "c", staged: false, untracked: true, status: "??" }),
    ]);
    expect(out.staged.map((f) => f.path)).toEqual(["a"]);
    expect(out.unstaged.map((f) => f.path)).toEqual(["b"]);
    expect(out.untracked.map((f) => f.path)).toEqual(["c"]);
  });

  it("preserves input order within each group", () => {
    const out = partitionFileChanges([
      fc({ path: "z", staged: true }),
      fc({ path: "a", staged: true }),
    ]);
    expect(out.staged.map((f) => f.path)).toEqual(["z", "a"]);
  });

  it("treats untracked as its own bucket regardless of staged flag", () => {
    const out = partitionFileChanges([
      fc({ path: "u", untracked: true, staged: false }),
    ]);
    expect(out.untracked).toHaveLength(1);
    expect(out.staged).toHaveLength(0);
  });
});

describe("summarizeBranch", () => {
  it("reports detached HEAD as label", () => {
    const s = summarizeBranch(status({ branch: null }));
    expect(s.label).toBe("(detached)");
  });

  it("treats no upstream as noUpstream + not in sync", () => {
    const s = summarizeBranch(status({ upstream: null }));
    expect(s.noUpstream).toBe(true);
    expect(s.inSync).toBe(false);
  });

  it("treats upstream + 0/0 as inSync", () => {
    const s = summarizeBranch(status());
    expect(s.inSync).toBe(true);
  });

  it("not inSync when ahead or behind", () => {
    expect(summarizeBranch(status({ ahead: 1 })).inSync).toBe(false);
    expect(summarizeBranch(status({ behind: 1 })).inSync).toBe(false);
  });
});

describe("labelFileStatus", () => {
  it("returns untracked when flag set", () => {
    expect(labelFileStatus(fc({ untracked: true, status: "??" }))).toBe(
      "untracked",
    );
  });

  it("prefers worktree letter over staged when present", () => {
    expect(labelFileStatus(fc({ status: ".M" }))).toBe("modified");
    expect(labelFileStatus(fc({ status: ".D" }))).toBe("deleted");
  });

  it("falls back to staged letter when worktree is .", () => {
    expect(labelFileStatus(fc({ status: "A." }))).toBe("added");
    expect(labelFileStatus(fc({ status: "M." }))).toBe("modified");
  });

  it("maps R / C / U / unknown", () => {
    expect(labelFileStatus(fc({ status: ".R" }))).toBe("renamed");
    expect(labelFileStatus(fc({ status: ".C" }))).toBe("copied");
    expect(labelFileStatus(fc({ status: ".U" }))).toBe("conflicted");
    expect(labelFileStatus(fc({ status: ".X" }))).toBe("changed");
  });
});

describe("relativeTime", () => {
  const now = Date.parse("2026-05-01T12:00:00Z");

  it("formats unix-seconds timestamps", () => {
    const t = Math.floor(now / 1000) - 30;
    expect(relativeTime(t, now)).toBe("30s ago");
  });

  it("formats ISO strings", () => {
    expect(relativeTime("2026-05-01T11:59:45Z", now)).toBe("15s ago");
  });

  it("buckets into minutes / hours / days", () => {
    const sec = Math.floor(now / 1000);
    expect(relativeTime(sec - 90, now)).toBe("1m ago");
    expect(relativeTime(sec - 3600 * 5, now)).toBe("5h ago");
    expect(relativeTime(sec - 86400 * 3, now)).toBe("3d ago");
  });

  it("clamps negative diffs to 0s", () => {
    const future = Math.floor(now / 1000) + 100;
    expect(relativeTime(future, now)).toBe("0s ago");
  });

  it("returns the raw input when ISO parsing fails", () => {
    expect(relativeTime("not-a-date", now)).toBe("not-a-date");
  });
});

describe("authorInitials", () => {
  it("returns first letter of first + last word", () => {
    expect(authorInitials(commit())).toBe("JD");
  });

  it("falls back to first two letters when only one word", () => {
    expect(authorInitials(commit({ author_name: "alice" }))).toBe("AL");
  });

  it("returns ? when name is empty / whitespace", () => {
    expect(authorInitials(commit({ author_name: "" }))).toBe("?");
    expect(authorInitials(commit({ author_name: "   " }))).toBe("?");
  });

  it("uppercases the result", () => {
    expect(authorInitials(commit({ author_name: "ada lovelace" }))).toBe("AL");
  });
});

describe("summarizeChangeCounts", () => {
  const f = (over: Partial<GitFileChange>): GitFileChange => ({
    path: "x",
    status: " M",
    staged: false,
    untracked: false,
    ...over,
  });

  it("is all-zero for an empty list", () => {
    expect(summarizeChangeCounts([])).toEqual({
      modified: 0,
      added: 0,
      deleted: 0,
      untracked: 0,
      conflicted: 0,
    });
  });

  it("tallies each kind, folding rename/copy into modified", () => {
    const counts = summarizeChangeCounts([
      f({ status: " M" }),
      f({ status: "R." }),
      f({ status: "A." }),
      f({ status: " D" }),
      f({ status: "??", untracked: true }),
      f({ status: "UU" }),
    ]);
    expect(counts).toEqual({
      modified: 2,
      added: 1,
      deleted: 1,
      untracked: 1,
      conflicted: 1,
    });
  });
});
