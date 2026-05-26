import { describe, it, expect, beforeEach, vi } from "vitest";

// Mock the Tauri invoke so we can capture the canonical text the hash is
// computed over without actually crossing the IPC boundary.
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: Record<string, unknown>) => invokeMock(cmd, args),
}));

import { computeHttpCacheHash, computeDbCacheHash } from "../hash";

beforeEach(() => {
  invokeMock.mockReset();
  // Echo the keyed input back as the "hash" so tests can assert exact
  // canonical text without locking onto a real digest.
  invokeMock.mockImplementation((_cmd: string, args: Record<string, unknown>) =>
    Promise.resolve(args.content as string),
  );
});

describe("computeHttpCacheHash", () => {
  const baseInput = {
    method: "GET" as const,
    url: "https://api.example.com/users",
    params: [
      { key: "page", value: "1" },
      { key: "limit", value: "10" },
    ],
    headers: [
      { key: "Authorization", value: "Bearer x" },
      { key: "Accept", value: "application/json" },
    ],
    body: "",
  };

  it("emits a deterministic canonical string for the same input", async () => {
    const a = await computeHttpCacheHash(baseInput, {});
    const b = await computeHttpCacheHash(baseInput, {});
    expect(a).toBe(b);
  });

  it("sorts query params alphabetically by key", async () => {
    const reordered = {
      ...baseInput,
      params: [
        { key: "limit", value: "10" },
        { key: "page", value: "1" },
      ],
    };
    const a = await computeHttpCacheHash(baseInput, {});
    const b = await computeHttpCacheHash(reordered, {});
    expect(a).toBe(b);
  });

  it("sorts headers case-insensitively by key", async () => {
    const reordered = {
      ...baseInput,
      headers: [
        { key: "accept", value: "application/json" },
        { key: "AUTHORIZATION", value: "Bearer x" },
      ],
    };
    const a = await computeHttpCacheHash(baseInput, {});
    const b = await computeHttpCacheHash(reordered, {});
    expect(a).toBe(b);
  });

  it("differentiates GET from POST on the same URL", async () => {
    const post = { ...baseInput, method: "POST" as const };
    const a = await computeHttpCacheHash(baseInput, {});
    const b = await computeHttpCacheHash(post, {});
    expect(a).not.toBe(b);
  });

  it("differentiates different URLs", async () => {
    const other = { ...baseInput, url: "https://api.example.com/posts" };
    const a = await computeHttpCacheHash(baseInput, {});
    const b = await computeHttpCacheHash(other, {});
    expect(a).not.toBe(b);
  });

  it("differentiates different bodies", async () => {
    const a = await computeHttpCacheHash({ ...baseInput, body: "hi" }, {});
    const b = await computeHttpCacheHash({ ...baseInput, body: "bye" }, {});
    expect(a).not.toBe(b);
  });

  it("URL-encodes query keys and values", async () => {
    const tricky = {
      ...baseInput,
      params: [{ key: "q", value: "foo bar/baz?qux" }],
    };
    const text = await computeHttpCacheHash(tricky, {});
    expect(text).toContain("q=foo%20bar%2Fbaz%3Fqux");
  });

  it("includes only env vars actually referenced in the request", async () => {
    const withRef = {
      ...baseInput,
      headers: [{ key: "Authorization", value: "Bearer {{TOKEN}}" }],
    };
    const env = { TOKEN: "abc", UNRELATED: "xyz" };
    const text = await computeHttpCacheHash(withRef, env);
    expect(text).toContain("__ENV__");
    expect(text).toContain("TOKEN=abc");
    expect(text).not.toContain("UNRELATED");
  });

  it("omits the env section entirely when no refs are present", async () => {
    const text = await computeHttpCacheHash(baseInput, { TOKEN: "abc" });
    expect(text).not.toContain("__ENV__");
  });

  it("changes when a referenced env var changes value", async () => {
    const refInput = {
      ...baseInput,
      url: "https://api.example.com/{{USER}}",
    };
    const a = await computeHttpCacheHash(refInput, { USER: "alice" });
    const b = await computeHttpCacheHash(refInput, { USER: "bob" });
    expect(a).not.toBe(b);
  });

  it("does NOT change when an unreferenced env var changes", async () => {
    const a = await computeHttpCacheHash(baseInput, { OTHER: "1" });
    const b = await computeHttpCacheHash(baseInput, { OTHER: "2" });
    expect(a).toBe(b);
  });

  it("passes connectionId as null (HTTP has no connection concept)", async () => {
    await computeHttpCacheHash(baseInput, {});
    expect(invokeMock).toHaveBeenCalledWith(
      "compute_block_hash",
      expect.objectContaining({ connectionId: null }),
    );
  });
});

describe("computeDbCacheHash (regression: HTTP changes don't break it)", () => {
  it("still hashes db bodies + connection + env-only-referenced", async () => {
    const a = await computeDbCacheHash("SELECT 1", "conn-x", {});
    expect(invokeMock).toHaveBeenCalledWith(
      "compute_block_hash",
      expect.objectContaining({ connectionId: "conn-x" }),
    );
    expect(a).toBe("SELECT 1");
  });
});
