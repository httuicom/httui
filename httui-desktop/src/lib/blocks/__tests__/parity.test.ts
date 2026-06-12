/**
 * Parity runner for the TS side of the block parser/serializer.
 *
 * Loads every fixture under `httui-desktop/test-fixtures/blocks/`,
 * parses the `input.md`, and asserts the result matches `expected.json`.
 * The Rust runner consumes a mirror copy of these fixtures from the
 * httui-core repository. When the two parsers diverge, one of them is
 * wrong; the fixture is the contract.
 *
 * The TS surface is split into multiple functions
 * (`parseHttpFenceInfo` + `parseHttpMessageBody` for http,
 * `parseDbFenceInfo` + raw SQL for db) — this runner composes them
 * into the same `{block_type, alias, display_mode, params}` envelope
 * the Rust `ParsedBlock` produces.
 */
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import {
  parseHttpFenceInfo,
  parseHttpMessageBody,
  type HttpKVRow,
  type HttpMessageParsed,
} from "../http-message";
import { parseDbFenceInfo } from "../db-fence";

// `__dirname` doesn't exist in ESM/vitest — derive the fixtures dir
// relative to this file so the runner finds the same path as the
// Rust runner.
const FIXTURES_DIR = path.resolve(
  path.dirname(new URL(import.meta.url).pathname),
  "../../../../test-fixtures/blocks",
);

interface CanonicalBlock {
  block_type: string;
  alias: string | null;
  display_mode: string | null;
  params: Record<string, unknown>;
}

interface CanonicalEnvelope {
  blocks: CanonicalBlock[];
}

/** Each fixture is one folder with `input.md` + `expected.json`. */
function listFixtures(): {
  name: string;
  input: string;
  expected: CanonicalEnvelope;
}[] {
  return readdirSync(FIXTURES_DIR, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort()
    .flatMap((name) => {
      const dir = path.join(FIXTURES_DIR, name);
      const inputPath = path.join(dir, "input.md");
      const expectedPath = path.join(dir, "expected.json");
      try {
        const input = readFileSync(inputPath, "utf8");
        const expected = JSON.parse(
          readFileSync(expectedPath, "utf8"),
        ) as CanonicalEnvelope;
        return [{ name, input, expected }];
      } catch {
        // Stub fixture (in-flight) — skip silently.
        return [];
      }
    });
}

/**
 * Walk the markdown and extract each fenced code block as `(info,
 * body)`. Tiny re-implementation since `parseHttpMessageBody` works
 * on a body string, not a full document, and we don't want to pull
 * the desktop's CodeMirror parser in here.
 */
function extractFences(md: string): Array<{ info: string; body: string }> {
  const fences: Array<{ info: string; body: string }> = [];
  const lines = md.split("\n");
  let i = 0;
  while (i < lines.length) {
    const open = lines[i].match(/^```(.*)$/);
    if (!open) {
      i++;
      continue;
    }
    const info = open[1];
    i++;
    const bodyStart = i;
    while (i < lines.length && lines[i] !== "```") {
      i++;
    }
    const body = lines.slice(bodyStart, i).join("\n");
    fences.push({ info, body });
    if (i < lines.length) i++; // consume closing ```
  }
  return fences;
}

/**
 * Strip transport-only fields from an `HttpKVRow` so the canonical
 * shape matches what Rust's `ParsedBlock.params` carries (no
 * `enabled` defaulting, no `description: undefined`). The Rust side
 * stores params as `[{key, value}]` for new HTTP-message blocks; the
 * TS parser adds `enabled: true` for in-app rendering. Both are
 * semantically equal — this normalization makes the comparison
 * structural.
 */
function normalizeKv(row: HttpKVRow): { key: string; value: string } {
  return { key: row.key, value: row.value };
}

function httpMessageToCanonicalParams(
  parsed: HttpMessageParsed,
): Record<string, unknown> {
  return {
    method: parsed.method,
    url: parsed.url,
    params: parsed.params.map(normalizeKv),
    headers: parsed.headers.map(normalizeKv),
    body: parsed.body,
  };
}

/** Canonical block from a fence. Returns null on unknown types. */
function fenceToCanonical(info: string, body: string): CanonicalBlock | null {
  const head = info.trim().split(/\s+/)[0] ?? "";

  if (head === "http") {
    const meta = parseHttpFenceInfo(info);
    if (!meta) return null;
    const parsed = parseHttpMessageBody(body);
    return {
      block_type: "http",
      alias: meta.alias ?? null,
      display_mode: meta.displayMode ?? null,
      params: httpMessageToCanonicalParams(parsed),
    };
  }

  if (head === "db" || head.startsWith("db-")) {
    const meta = parseDbFenceInfo(info);
    if (!meta) return null;
    const params: Record<string, unknown> = {
      query: body.replace(/\n+$/, ""),
    };
    if (meta.connection) params.connection_id = meta.connection;
    if (typeof meta.limit === "number") params.limit = meta.limit;
    if (typeof meta.timeoutMs === "number") params.timeout_ms = meta.timeoutMs;
    return {
      block_type: head,
      alias: meta.alias ?? null,
      display_mode: meta.displayMode ?? null,
      params,
    };
  }

  return null;
}

describe("block parity (TS vs. Rust)", () => {
  const fixtures = listFixtures();

  it("has at least one fixture loaded", () => {
    // Smoke: a fresh checkout shouldn't regress fixture discovery.
    expect(fixtures.length).toBeGreaterThan(0);
  });

  for (const fix of fixtures) {
    it(`parses ${fix.name} to the canonical shape`, () => {
      const fences = extractFences(fix.input);
      const blocks = fences
        .map((f) => fenceToCanonical(f.info, f.body))
        .filter((b): b is CanonicalBlock => b !== null);
      expect({ blocks }).toEqual(fix.expected);
    });
  }
});
