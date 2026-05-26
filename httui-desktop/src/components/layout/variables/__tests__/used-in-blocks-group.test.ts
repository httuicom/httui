import { describe, expect, it } from "vitest";

import { groupVarUsesByFile } from "@/components/layout/variables/used-in-blocks-group";
import type { VarUseEntry } from "@/lib/tauri/var-uses";

function entry(file: string, line: number, snippet = ""): VarUseEntry {
  return { file_path: file, line, snippet };
}

describe("groupVarUsesByFile", () => {
  it("returns an empty list when no entries are passed", () => {
    expect(groupVarUsesByFile([])).toEqual([]);
  });

  it("groups consecutive same-file entries into one group", () => {
    const groups = groupVarUsesByFile([
      entry("a.md", 1, "x"),
      entry("a.md", 5, "y"),
    ]);
    expect(groups).toEqual([
      {
        filePath: "a.md",
        hits: [
          { line: 1, snippet: "x" },
          { line: 5, snippet: "y" },
        ],
      },
    ]);
  });

  it("preserves input order across files (already sorted by Rust)", () => {
    const groups = groupVarUsesByFile([
      entry("a.md", 2),
      entry("a.md", 7),
      entry("m.md", 1),
      entry("z.md", 1),
      entry("z.md", 4),
    ]);
    expect(groups.map((g) => g.filePath)).toEqual(["a.md", "m.md", "z.md"]);
    expect(groups[0].hits.length).toBe(2);
    expect(groups[1].hits.length).toBe(1);
    expect(groups[2].hits.length).toBe(2);
  });

  it("starts a new group when the file path changes (even briefly)", () => {
    // The Rust side sorts by file path, but the helper shouldn't
    // assume that — it groups consecutive runs only.
    const groups = groupVarUsesByFile([
      entry("a.md", 1),
      entry("b.md", 1),
      entry("a.md", 2),
    ]);
    expect(groups.length).toBe(3);
    expect(groups.map((g) => g.filePath)).toEqual(["a.md", "b.md", "a.md"]);
  });
});
