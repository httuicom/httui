import { describe, expect, it } from "vitest";

import { resolveFileTreeDrop } from "@/components/layout/file-tree/file-tree-drag";
import type { DragEndEvent } from "@dnd-kit/core";

function event(over: object | null, active: object): DragEndEvent {
  return { active, over } as unknown as DragEndEvent;
}

function activeWith(id: string, path?: string) {
  return { id, data: { current: path !== undefined ? { path } : {} } };
}

function overWith(id: string, dirPath?: string) {
  return { id, data: { current: dirPath !== undefined ? { dirPath } : {} } };
}

describe("resolveFileTreeDrop", () => {
  it("returns null when there is no drop target", () => {
    expect(
      resolveFileTreeDrop(event(null, activeWith("a", "a.md"))),
    ).toBeNull();
  });

  it("returns null when the drop target is the source itself", () => {
    expect(
      resolveFileTreeDrop(
        event(overWith("a", "a.md"), activeWith("a", "a.md")),
      ),
    ).toBeNull();
  });

  it("returns null when active has no path data", () => {
    expect(
      resolveFileTreeDrop(
        event(overWith("dir", "dir"), { id: "a", data: { current: {} } }),
      ),
    ).toBeNull();
  });

  it("returns null when over has no dirPath data", () => {
    expect(
      resolveFileTreeDrop(event(overWith("dir"), activeWith("a", "a.md"))),
    ).toBeNull();
  });

  it("returns null when source equals target dir (drop on own dir)", () => {
    expect(
      resolveFileTreeDrop(
        event(overWith("dir", "notes"), activeWith("notes", "notes")),
      ),
    ).toBeNull();
  });

  it("returns null when target dir is a descendant of the source", () => {
    expect(
      resolveFileTreeDrop(
        event(overWith("subdir", "notes/subdir"), activeWith("notes", "notes")),
      ),
    ).toBeNull();
  });

  it("returns the resolved {source, target} for a clean cross-directory move", () => {
    expect(
      resolveFileTreeDrop(
        event(
          overWith("archive", "archive"),
          activeWith("note", "drafts/note.md"),
        ),
      ),
    ).toEqual({ sourcePath: "drafts/note.md", targetDir: "archive" });
  });

  it("accepts an empty-string targetDir (root drop)", () => {
    expect(
      resolveFileTreeDrop(
        event(overWith("root", ""), activeWith("note", "drafts/note.md")),
      ),
    ).toEqual({ sourcePath: "drafts/note.md", targetDir: "" });
  });
});
