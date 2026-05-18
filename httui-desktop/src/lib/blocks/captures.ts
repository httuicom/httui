// Block captures — (parser + evaluator).
//
// Mirrors the assertions fence-section shape `# capture:`
// marker section after the body, with `<key> = <expr>` lines that
// extract values from the response. Reuses `resolveLhs` from
// assertions.ts so both features share the same JSONPath subset.
//
// Storage example:
//   ```http alias=login
//   POST /auth
//
//   # capture:
//   # token = $.body.access_token
//   # user_id = $.body.user.id
//   ```

import { resolveLhs, type AssertionContext } from "./assertions";

const CAPTURE_MARKER = /^\s*#\s*capture\s*:\s*$/i;
const COMMENT_PREFIX = /^\s*#\s?/;
/** Same constraints as variable names — no whitespace / no dot. */
const KEY_REGEX = /^[A-Za-z_$][A-Za-z0-9_$-]*$/;

export interface ParsedCapture {
  /** 1-indexed line offset within the block body. */
  line: number;
  raw: string;
  key: string;
  /** Right-hand expression (e.g. `$.body.access_token`, `status`). */
  expr: string;
}

export type CaptureContext = AssertionContext;

/** Walk the block body and return the lines under `# capture:`. */
export function extractCaptureLines(blockBody: string): {
  rawLine: string;
  bodyLine: number;
}[] {
  const lines = blockBody.split("\n");
  const out: { rawLine: string; bodyLine: number }[] = [];
  let inSection = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!inSection) {
      if (CAPTURE_MARKER.test(line)) inSection = true;
      continue;
    }
    if (line.trim() === "") break;
    if (!COMMENT_PREFIX.test(line)) break;
    out.push({
      rawLine: line.replace(COMMENT_PREFIX, ""),
      bodyLine: i + 1,
    });
  }
  return out;
}

/** Parse a single `<key> = <expr>` line. Returns null when the line
 * doesn't fit the shape OR when the key fails validation. */
export function parseCaptureLine(
  rawLine: string,
  bodyLine: number,
): ParsedCapture | null {
  const trimmed = rawLine.trim();
  if (!trimmed) return null;
  const eq = trimmed.indexOf("=");
  if (eq < 0) return null;
  const key = trimmed.slice(0, eq).trim();
  const expr = trimmed.slice(eq + 1).trim();
  if (!key || !expr) return null;
  if (!KEY_REGEX.test(key)) return null;
  return { line: bodyLine, raw: trimmed, key, expr };
}

export function parseAllCaptures(blockBody: string): ParsedCapture[] {
  const lines = extractCaptureLines(blockBody);
  const out: ParsedCapture[] = [];
  for (const { rawLine, bodyLine } of lines) {
    const parsed = parseCaptureLine(rawLine, bodyLine);
    if (parsed) out.push(parsed);
  }
  return out;
}

/** Evaluate every capture against `ctx`. Each captured key maps to
 * the resolved value (which may be `undefined` when the path doesn't
 * exist — the consumer decides whether to drop, mask, or keep). */
export function evaluateCaptures(
  captures: ReadonlyArray<ParsedCapture>,
  ctx: CaptureContext,
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const c of captures) {
    out[c.key] = resolveLhs(c.expr, ctx);
  }
  return out;
}

/** Convenience: parse + evaluate in one call. */
export function captureValuesFromBody(
  blockBody: string,
  ctx: CaptureContext,
): Record<string, unknown> {
  return evaluateCaptures(parseAllCaptures(blockBody), ctx);
}

/** Privacy guard (preview — used by the store on insert). */
const SECRET_NAME_REGEX = /(password|token|secret|key|auth\w*)/i;

/** True when `name` looks like a secret by convention. */
export function isSecretCaptureKey(name: string): boolean {
  return SECRET_NAME_REGEX.test(name);
}
