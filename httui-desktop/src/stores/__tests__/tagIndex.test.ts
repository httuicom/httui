import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { clearTauriMocks, mockTauriCommand } from "@/test/mocks/tauri";

import { useTagIndexStore } from "../tagIndex";

beforeEach(() => {
  useTagIndexStore.getState().clearAll();
});

afterEach(() => {
  clearTauriMocks();
});

describe("tagIndex store", () => {
  it("starts empty", () => {
    const s = useTagIndexStore.getState();
    expect(s.getAllTags()).toEqual([]);
    expect(s.getFilesByTag("anything")).toEqual([]);
  });

  it("indexes a file's tags on setTagsForFile", () => {
    useTagIndexStore.getState().setTagsForFile("a.md", ["payments", "debug"]);
    const s = useTagIndexStore.getState();
    expect(s.getAllTags()).toEqual(["debug", "payments"]);
    expect(s.getFilesByTag("payments")).toEqual(["a.md"]);
    expect(s.getFilesByTag("debug")).toEqual(["a.md"]);
  });

  it("merges multiple files under the same tag", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["x"]);
    s.setTagsForFile("b.md", ["x"]);
    expect(useTagIndexStore.getState().getFilesByTag("x")).toEqual([
      "a.md",
      "b.md",
    ]);
  });

  it("setTagsForFile replaces the previous tag set for that file", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["x", "y"]);
    s.setTagsForFile("a.md", ["y", "z"]);
    const after = useTagIndexStore.getState();
    expect(after.getAllTags()).toEqual(["y", "z"]);
    expect(after.getFilesByTag("x")).toEqual([]);
    expect(after.getFilesByTag("y")).toEqual(["a.md"]);
    expect(after.getFilesByTag("z")).toEqual(["a.md"]);
  });

  it("removes orphaned tag entries when no files reference them", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["x"]);
    s.setTagsForFile("a.md", []);
    expect(useTagIndexStore.getState().getAllTags()).toEqual([]);
  });

  it("removeFile drops the file from every tag bucket", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["x"]);
    s.setTagsForFile("b.md", ["x", "y"]);
    s.removeFile("a.md");
    const after = useTagIndexStore.getState();
    expect(after.getFilesByTag("x")).toEqual(["b.md"]);
    expect(after.getFilesByTag("y")).toEqual(["b.md"]);
    expect(after.getAllTags()).toEqual(["x", "y"]);
  });

  it("removeFile is a no-op for unknown files", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["x"]);
    const before = useTagIndexStore.getState();
    s.removeFile("nope.md");
    const after = useTagIndexStore.getState();
    expect(after.byTag).toEqual(before.byTag);
    expect(after.byFile).toEqual(before.byFile);
  });

  it("removeFile drops orphaned tags entirely", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["unique"]);
    s.removeFile("a.md");
    expect(useTagIndexStore.getState().getAllTags()).toEqual([]);
  });

  it("clearAll resets everything", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["x"]);
    s.setTagsForFile("b.md", ["y"]);
    s.clearAll();
    const after = useTagIndexStore.getState();
    expect(after.byTag).toEqual({});
    expect(after.byFile).toEqual({});
  });

  it("getAllTags returns sorted output", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("a.md", ["zebra", "alpha", "mango"]);
    expect(useTagIndexStore.getState().getAllTags()).toEqual([
      "alpha",
      "mango",
      "zebra",
    ]);
  });

  it("getFilesByTag returns sorted output", () => {
    const s = useTagIndexStore.getState();
    s.setTagsForFile("zebra.md", ["t"]);
    s.setTagsForFile("alpha.md", ["t"]);
    s.setTagsForFile("mango.md", ["t"]);
    expect(useTagIndexStore.getState().getFilesByTag("t")).toEqual([
      "alpha.md",
      "mango.md",
      "zebra.md",
    ]);
  });

  it("loadFromVault populates the index from scan_vault_tags_cmd", async () => {
    mockTauriCommand("scan_vault_tags_cmd", () => [
      { path: "alpha.md", tags: ["payments", "debug"] },
      { path: "beta.md", tags: ["payments"] },
    ]);
    const count = await useTagIndexStore.getState().loadFromVault("/v");
    expect(count).toBe(2);
    const s = useTagIndexStore.getState();
    expect(s.getAllTags()).toEqual(["debug", "payments"]);
    expect(s.getFilesByTag("payments")).toEqual(["alpha.md", "beta.md"]);
    expect(s.getFilesByTag("debug")).toEqual(["alpha.md"]);
  });

  it("loadFromVault forwards the vaultPath argument", async () => {
    let captured: unknown;
    mockTauriCommand("scan_vault_tags_cmd", (args) => {
      captured = args;
      return [];
    });
    await useTagIndexStore.getState().loadFromVault("/some/vault/root");
    expect(captured).toEqual({ vaultPath: "/some/vault/root" });
  });

  it("loadFromVault returns 0 when the walker reports nothing", async () => {
    mockTauriCommand("scan_vault_tags_cmd", () => []);
    const count = await useTagIndexStore.getState().loadFromVault("/v");
    expect(count).toBe(0);
    expect(useTagIndexStore.getState().getAllTags()).toEqual([]);
  });

  it("loadFromVault replaces every existing entry (no merge)", async () => {
    // Stale state from a prior scan should not survive the next
    // bootstrap — vault-switch needs to start clean.
    useTagIndexStore.getState().setTagsForFile("stale.md", ["legacy"]);
    mockTauriCommand("scan_vault_tags_cmd", () => [
      { path: "fresh.md", tags: ["next"] },
    ]);
    await useTagIndexStore.getState().loadFromVault("/v");
    const s = useTagIndexStore.getState();
    expect(Object.keys(s.byFile)).toEqual(["fresh.md"]);
    expect(s.getAllTags()).toEqual(["next"]);
    expect(s.getFilesByTag("legacy")).toEqual([]);
  });

  it("loadFromVault dedups same-tag-twice within a file", async () => {
    mockTauriCommand("scan_vault_tags_cmd", () => [
      { path: "a.md", tags: ["foo", "foo", "bar"] },
    ]);
    await useTagIndexStore.getState().loadFromVault("/v");
    const s = useTagIndexStore.getState();
    expect(s.byFile["a.md"]).toEqual(["foo", "bar"]);
    expect(s.getAllTags()).toEqual(["bar", "foo"]);
  });

  it("loadFromVault skips defensively malformed entries", async () => {
    mockTauriCommand("scan_vault_tags_cmd", () => [
      { path: "good.md", tags: ["ok"] },
      null,
      { path: 42, tags: ["x"] },
      { path: "bad.md", tags: "not-array" },
      { path: "empty-tags.md", tags: [] },
    ]);
    const count = await useTagIndexStore.getState().loadFromVault("/v");
    expect(count).toBe(2); // good.md + empty-tags.md (kept; just no tag bucket)
    const s = useTagIndexStore.getState();
    expect(s.byFile["good.md"]).toEqual(["ok"]);
    expect(s.getAllTags()).toEqual(["ok"]);
    expect(s.getFilesByTag("ok")).toEqual(["good.md"]);
  });

  it("loadFromVault batches into a single render via one set call", async () => {
    let renderCount = 0;
    const unsub = useTagIndexStore.subscribe(() => {
      renderCount += 1;
    });
    try {
      mockTauriCommand("scan_vault_tags_cmd", () =>
        Array.from({ length: 50 }, (_, i) => ({
          path: `f${i}.md`,
          tags: ["t"],
        })),
      );
      await useTagIndexStore.getState().loadFromVault("/v");
      expect(renderCount).toBe(1);
    } finally {
      unsub();
    }
  });

  it("loadFromVault propagates Tauri rejections", async () => {
    mockTauriCommand("scan_vault_tags_cmd", () => {
      throw new Error("vault not found");
    });
    await expect(
      useTagIndexStore.getState().loadFromVault("/missing"),
    ).rejects.toThrow("vault not found");
  });

  it("refreshTagsForFile parses content and indexes the tags", () => {
    const content = "---\ntags: [payments, debug]\n---\nbody\n";
    const tags = useTagIndexStore
      .getState()
      .refreshTagsForFile("a.md", content);
    expect(tags).toEqual(["payments", "debug"]);
    const s = useTagIndexStore.getState();
    expect(s.getFilesByTag("payments")).toEqual(["a.md"]);
    expect(s.getFilesByTag("debug")).toEqual(["a.md"]);
  });

  it("refreshTagsForFile clears tags when frontmatter is removed", () => {
    // First save: file has tags.
    useTagIndexStore
      .getState()
      .refreshTagsForFile("a.md", "---\ntags: [a, b]\n---\nbody\n");
    expect(useTagIndexStore.getState().getAllTags()).toEqual(["a", "b"]);

    // Subsequent save: user dropped the frontmatter — tags collapse.
    const tags = useTagIndexStore
      .getState()
      .refreshTagsForFile("a.md", "no frontmatter body only\n");
    expect(tags).toEqual([]);
    expect(useTagIndexStore.getState().getAllTags()).toEqual([]);
  });

  it("refreshTagsForFile flips the file's tag set on each save", () => {
    const s = useTagIndexStore.getState();
    s.refreshTagsForFile("a.md", "---\ntags: [old]\n---\n");
    s.refreshTagsForFile("a.md", "---\ntags: [new]\n---\n");
    const after = useTagIndexStore.getState();
    expect(after.getFilesByTag("old")).toEqual([]);
    expect(after.getFilesByTag("new")).toEqual(["a.md"]);
  });

  it("refreshTagsForFile is a no-op for content without frontmatter", () => {
    const tags = useTagIndexStore
      .getState()
      .refreshTagsForFile("a.md", "# Just markdown\n\nbody\n");
    expect(tags).toEqual([]);
    expect(useTagIndexStore.getState().byFile["a.md"]).toEqual([]);
  });

  describe("archived files", () => {
    it("starts with no archived files", () => {
      expect(useTagIndexStore.getState().archivedFiles).toEqual({});
      expect(useTagIndexStore.getState().isArchived("a.md")).toBe(false);
    });

    it("setArchivedForFile flips the per-file archived flag", () => {
      const s = useTagIndexStore.getState();
      s.setArchivedForFile("a.md", true);
      expect(useTagIndexStore.getState().isArchived("a.md")).toBe(true);
      s.setArchivedForFile("a.md", false);
      expect(useTagIndexStore.getState().isArchived("a.md")).toBe(false);
    });

    it("setArchivedForFile is idempotent (no state churn when unchanged)", () => {
      const s = useTagIndexStore.getState();
      s.setArchivedForFile("a.md", true);
      const snapshot = useTagIndexStore.getState().archivedFiles;
      s.setArchivedForFile("a.md", true);
      expect(useTagIndexStore.getState().archivedFiles).toBe(snapshot);
    });

    it("refreshTagsForFile sets archived when status: archived", () => {
      useTagIndexStore
        .getState()
        .refreshTagsForFile("a.md", "---\nstatus: archived\n---\n");
      expect(useTagIndexStore.getState().isArchived("a.md")).toBe(true);
    });

    it("refreshTagsForFile clears archived when status changes away", () => {
      const s = useTagIndexStore.getState();
      s.refreshTagsForFile("a.md", "---\nstatus: archived\n---\n");
      s.refreshTagsForFile("a.md", "---\nstatus: active\n---\n");
      expect(useTagIndexStore.getState().isArchived("a.md")).toBe(false);
    });

    it("removeFile drops the archived flag", () => {
      const s = useTagIndexStore.getState();
      s.setArchivedForFile("a.md", true);
      s.removeFile("a.md");
      expect(useTagIndexStore.getState().isArchived("a.md")).toBe(false);
      expect(useTagIndexStore.getState().archivedFiles).toEqual({});
    });

    it("clearAll wipes archived state too", () => {
      const s = useTagIndexStore.getState();
      s.setArchivedForFile("a.md", true);
      s.setArchivedForFile("b.md", true);
      s.clearAll();
      expect(useTagIndexStore.getState().archivedFiles).toEqual({});
    });

    it("archived + tagged in same file: both indexed, both cleared on remove", () => {
      const s = useTagIndexStore.getState();
      s.refreshTagsForFile("a.md", "---\nstatus: archived\ntags: [api]\n---\n");
      const after = useTagIndexStore.getState();
      expect(after.isArchived("a.md")).toBe(true);
      expect(after.getFilesByTag("api")).toEqual(["a.md"]);
      after.removeFile("a.md");
      const final = useTagIndexStore.getState();
      expect(final.isArchived("a.md")).toBe(false);
      expect(final.getFilesByTag("api")).toEqual([]);
    });
  });
});
