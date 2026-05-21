import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";
import { GIT_STATUS_POLL_MS, resetGitStore, useGitStore } from "@/stores/git";
import type { GitStatus } from "@/lib/tauri/git";

const STATUS: GitStatus = {
  branch: "main",
  upstream: "origin/main",
  ahead: 0,
  behind: 0,
  changed: [],
  clean: true,
};
const REMOTE = { name: "origin", url: "git@github.com:foo/bar.git" };
const COMMIT = {
  sha: "abc123",
  short_sha: "abc123",
  author_name: "Ada",
  author_email: "ada@x.dev",
  timestamp: 1,
  subject: "init",
};

const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

const st = () => useGitStore.getState();

beforeEach(() => {
  clearTauriMocks();
  resetGitStore();
  vi.useFakeTimers();
});

afterEach(() => {
  clearTauriMocks();
  resetGitStore();
  vi.useRealTimers();
});

describe("useGitStore", () => {
  it("starts from a clean initial state", () => {
    expect(st().vaultPath).toBeNull();
    expect(st().status).toBeNull();
    expect(st().remotes).toEqual([]);
    expect(st().remotesLoaded).toBe(false);
    expect(st().commits).toEqual([]);
    expect(st().commitMessage).toBe("");
    expect(st().commitMessageDirty).toBe(false);
    expect(st().lastSyncAt).toBeNull();
    expect(st()._subscribers).toBe(0);
    expect(st()._timer).toBeNull();
  });

  describe("acquire / release", () => {
    it("acquire(null) bumps subscribers but starts no poll timer", () => {
      st().acquire(null);
      expect(st()._subscribers).toBe(1);
      expect(st()._timer).toBeNull();
    });

    it("acquire(path) fetches immediately and starts a poll timer", async () => {
      let statusCalls = 0;
      mockTauriCommand("git_status_cmd", () => {
        statusCalls += 1;
        return STATUS;
      });
      mockTauriCommand("git_remote_list_cmd", () => [REMOTE]);

      st().acquire("/v");
      await flush();

      expect(statusCalls).toBe(1);
      expect(st().status).toEqual(STATUS);
      expect(st().remotes).toEqual([REMOTE]);
      expect(st()._timer).not.toBeNull();
    });

    it("polls again after GIT_STATUS_POLL_MS", async () => {
      let calls = 0;
      mockTauriCommand("git_status_cmd", () => {
        calls += 1;
        return STATUS;
      });
      mockTauriCommand("git_remote_list_cmd", () => []);

      st().acquire("/v");
      await flush();
      expect(calls).toBe(1);

      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await flush();
      expect(calls).toBe(2);
    });

    it("a second acquire on the same path keeps one timer and prior data", async () => {
      mockTauriCommand("git_status_cmd", () => STATUS);
      mockTauriCommand("git_remote_list_cmd", () => []);

      st().acquire("/v");
      await flush();
      const timer = st()._timer;

      st().acquire("/v");
      expect(st()._subscribers).toBe(2);
      expect(st()._timer).toBe(timer);
      expect(st().status).toEqual(STATUS);
    });

    it("acquire on a new path resets data to initial", async () => {
      mockTauriCommand("git_status_cmd", () => STATUS);
      mockTauriCommand("git_remote_list_cmd", () => [REMOTE]);

      st().acquire("/v");
      await flush();
      expect(st().status).toEqual(STATUS);

      st().acquire("/other");
      expect(st().vaultPath).toBe("/other");
      expect(st().status).toBeNull();
      expect(st().remotes).toEqual([]);
    });

    it("release stops the timer only when the last subscriber leaves", async () => {
      mockTauriCommand("git_status_cmd", () => STATUS);
      mockTauriCommand("git_remote_list_cmd", () => []);

      st().acquire("/v");
      st().acquire("/v");
      await flush();
      expect(st()._timer).not.toBeNull();

      st().release();
      expect(st()._subscribers).toBe(1);
      expect(st()._timer).not.toBeNull();

      st().release();
      expect(st()._subscribers).toBe(0);
      expect(st()._timer).toBeNull();
    });

    it("release never drives subscribers below zero", () => {
      st().release();
      expect(st()._subscribers).toBe(0);
    });
  });

  describe("refreshStatus", () => {
    it("clears status when no vault is set", async () => {
      useGitStore.setState({ status: STATUS, statusError: "x" });
      await st().refreshStatus();
      expect(st().status).toBeNull();
      expect(st().statusError).toBeNull();
    });

    it("keeps the previous status visible on a failed poll", async () => {
      useGitStore.setState({ vaultPath: "/v", status: STATUS });
      mockTauriCommand("git_status_cmd", () => {
        throw new Error("boom");
      });
      await st().refreshStatus();
      expect(st().statusError).toBe("boom");
      expect(st().status).toEqual(STATUS);
    });

    it("ignores a response that resolves after the vault switched", async () => {
      let resolveFn: ((v: GitStatus) => void) | null = null;
      mockTauriCommand(
        "git_status_cmd",
        () => new Promise((res) => (resolveFn = res as typeof resolveFn)),
      );
      useGitStore.setState({ vaultPath: "/v" });
      const p = st().refreshStatus();
      useGitStore.setState({ vaultPath: "/switched" });
      resolveFn!(STATUS);
      await p;
      expect(st().status).toBeNull();
    });
  });

  describe("refreshRemotes", () => {
    it("clears remotes when no vault is set", async () => {
      useGitStore.setState({ remotes: [REMOTE], remotesLoaded: true });
      await st().refreshRemotes();
      expect(st().remotes).toEqual([]);
      expect(st().remotesLoaded).toBe(false);
    });

    it("treats an empty list as a successful load", async () => {
      useGitStore.setState({ vaultPath: "/v" });
      mockTauriCommand("git_remote_list_cmd", () => []);
      await st().refreshRemotes();
      expect(st().remotes).toEqual([]);
      expect(st().remotesLoaded).toBe(true);
      expect(st().remotesError).toBeNull();
    });

    it("surfaces errors and clears the loaded flag", async () => {
      useGitStore.setState({ vaultPath: "/v" });
      mockTauriCommand("git_remote_list_cmd", () => {
        throw "nope";
      });
      await st().refreshRemotes();
      expect(st().remotes).toEqual([]);
      expect(st().remotesLoaded).toBe(false);
      expect(st().remotesError).toBe("nope");
    });
  });

  describe("reloadLog", () => {
    it("clears commits when no vault is set", async () => {
      useGitStore.setState({ commits: [COMMIT] });
      await st().reloadLog();
      expect(st().commits).toEqual([]);
    });

    it("loads commits and forwards the path filter", async () => {
      useGitStore.setState({ vaultPath: "/v" });
      let seenFilter: unknown;
      mockTauriCommand("git_log_cmd", (args) => {
        seenFilter = (args as { pathFilter: unknown }).pathFilter;
        return [COMMIT];
      });
      await st().reloadLog("notes/");
      expect(st().commits).toEqual([COMMIT]);
      expect(seenFilter).toBe("notes/");
    });

    it("empties the list on a transient failure", async () => {
      useGitStore.setState({ vaultPath: "/v", commits: [COMMIT] });
      mockTauriCommand("git_log_cmd", () => {
        throw new Error("not a repo");
      });
      await st().reloadLog();
      expect(st().commits).toEqual([]);
    });
  });

  describe("commit draft", () => {
    it("setCommitMessage marks the draft dirty", () => {
      st().setCommitMessage("hello");
      expect(st().commitMessage).toBe("hello");
      expect(st().commitMessageDirty).toBe(true);
    });

    it("resetCommitMessage clears the draft and the dirty flag", () => {
      st().setCommitMessage("hello");
      st().resetCommitMessage();
      expect(st().commitMessage).toBe("");
      expect(st().commitMessageDirty).toBe(false);
    });

    it("setCommitMessageFromTemplate sets text but keeps it non-dirty", () => {
      st().setCommitMessageFromTemplate("Update foo");
      expect(st().commitMessage).toBe("Update foo");
      expect(st().commitMessageDirty).toBe(false);
    });

    it("markSynced stamps lastSyncAt with the current time", () => {
      const before = Date.now();
      st().markSynced();
      expect(st().lastSyncAt).not.toBeNull();
      expect(st().lastSyncAt!).toBeGreaterThanOrEqual(before);
    });
  });

  describe("poll ref-stability (H2)", () => {
    // The real IPC deserializes a fresh object every tick; mimic that
    // so the test exercises the structural-equality guard (not mock
    // reference identity).
    const freshStatus = () => JSON.parse(JSON.stringify(STATUS)) as GitStatus;

    it("keeps the same status + remotes refs across an unchanged poll", async () => {
      mockTauriCommand("git_status_cmd", () => freshStatus());
      mockTauriCommand("git_remote_list_cmd", () => [{ ...REMOTE }]);

      st().acquire("/v");
      await flush();
      const statusRef = st().status;
      const remotesRef = st().remotes;
      expect(statusRef).toEqual(STATUS);

      const notified = vi.fn();
      const unsub = useGitStore.subscribe(notified);
      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await flush();
      unsub();

      // Same data → same references → no subscriber would re-render.
      expect(st().status).toBe(statusRef);
      expect(st().remotes).toBe(remotesRef);
      expect(notified).not.toHaveBeenCalled();
    });

    it("swaps the status ref when git actually changes", async () => {
      let clean = true;
      mockTauriCommand("git_status_cmd", () => ({
        ...freshStatus(),
        clean,
      }));
      mockTauriCommand("git_remote_list_cmd", () => []);

      st().acquire("/v");
      await flush();
      const statusRef = st().status;

      clean = false; // a file changed between polls
      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await flush();

      expect(st().status).not.toBe(statusRef);
      expect(st().status?.clean).toBe(false);
    });

    it("swaps the remotes ref when a remote is added", async () => {
      let list: { name: string; url: string }[] = [];
      mockTauriCommand("git_status_cmd", () => freshStatus());
      mockTauriCommand("git_remote_list_cmd", () =>
        list.map((r) => ({ ...r })),
      );

      st().acquire("/v");
      await flush();
      const remotesRef = st().remotes;
      expect(remotesRef).toEqual([]);

      list = [REMOTE];
      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await flush();

      expect(st().remotes).not.toBe(remotesRef);
      expect(st().remotes).toEqual([REMOTE]);
    });

    it("clears a stale statusError on an unchanged poll without churning the ref", async () => {
      mockTauriCommand("git_status_cmd", () => freshStatus());
      mockTauriCommand("git_remote_list_cmd", () => []);

      st().acquire("/v");
      await flush();
      const statusRef = st().status;
      useGitStore.setState({ statusError: "stale boom" });

      vi.advanceTimersByTime(GIT_STATUS_POLL_MS);
      await flush();

      expect(st().statusError).toBeNull();
      expect(st().status).toBe(statusRef);
    });
  });

  it("resetGitStore tears down a live poll timer", async () => {
    mockTauriCommand("git_status_cmd", () => STATUS);
    mockTauriCommand("git_remote_list_cmd", () => []);
    st().acquire("/v");
    await flush();
    expect(st()._timer).not.toBeNull();

    resetGitStore();
    expect(st()._timer).toBeNull();
    expect(st().vaultPath).toBeNull();
  });
});
