// Read-side cache resolution for `{{alias…}}`: HTTP rows are keyed by a
// request+env hash the scanner cannot reproduce, so blocks must resolve
// by alias — a content-hash lookup would miss every row.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Text } from "@codemirror/state";

const getBlockResult = vi.fn();
const getLatestBlockResultByAlias = vi.fn();
vi.mock("@/lib/tauri/commands", () => ({
  getBlockResult: (...args: unknown[]) => getBlockResult(...args),
  getLatestBlockResultByAlias: (...args: unknown[]) =>
    getLatestBlockResultByAlias(...args),
}));
vi.mock("@/lib/tauri/connections", () => ({
  listConnections: vi.fn(async () => []),
}));
vi.mock("@/stores/environment", () => ({
  useEnvironmentStore: {
    getState: () => ({ getActiveVariables: async () => ({}) }),
  },
}));

import { collectBlocksAboveCM } from "../document";

const row = {
  status: "success",
  response: '{"status_code":200}',
  total_rows: null,
  elapsed_ms: 5,
  executed_at: "",
};

beforeEach(() => {
  getBlockResult.mockReset();
  getLatestBlockResultByAlias.mockReset();
});

describe("collectBlocksAboveCM", () => {
  const doc = Text.of([
    "```http alias=req1",
    "GET https://api.example.com/users",
    "```",
    "",
    "```http alias=req2",
    "GET https://x.dev/{{req1.response.body.id}}",
    "```",
    "",
  ]);

  it("resolves http results by alias, not content hash", async () => {
    getLatestBlockResultByAlias.mockResolvedValue(row);
    const blocks = await collectBlocksAboveCM(doc, doc.length, "f.md");
    expect(blocks.map((b) => b.alias)).toEqual(["req1", "req2"]);
    expect(getLatestBlockResultByAlias).toHaveBeenCalledWith("f.md", "req1");
    expect(getBlockResult).not.toHaveBeenCalled();
    expect(blocks[0].cachedResult).toEqual({
      status: "success",
      response: '{"status_code":200}',
    });
  });

  it("leaves cachedResult null when no row exists for the alias", async () => {
    getLatestBlockResultByAlias.mockResolvedValue(null);
    const blocks = await collectBlocksAboveCM(doc, doc.length, "f.md");
    expect(blocks[0].cachedResult).toBeNull();
  });

  it("db block without a resolvable connection falls back to the alias row", async () => {
    const dbDoc = Text.of([
      "```db-postgres alias=q1 connection=ghost",
      "SELECT 1;",
      "```",
      "",
    ]);
    getLatestBlockResultByAlias.mockResolvedValue(row);
    const blocks = await collectBlocksAboveCM(dbDoc, dbDoc.length, "f.md");
    expect(blocks).toHaveLength(1);
    expect(getLatestBlockResultByAlias).toHaveBeenCalledWith("f.md", "q1");
    expect(blocks[0].cachedResult?.status).toBe("success");
  });
});
