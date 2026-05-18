// Block captures session store.
//
// In-memory only by default. ships a separate persistence
// path (`.captures.json`) for auto-capture mode; this store keeps the
// session-scoped values so subsequent blocks can resolve
// `{{<alias>.captures.<key>}}` against the previous run's output.
//
// Shape: `Record<file_path, Record<alias, Record<key, value>>>`. Plain
// records (not Map) so React/Zustand reactivity works without manual
// replace.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

import { isSecretCaptureKey } from "@/lib/blocks/captures";

export type CapturedValue = string | number | boolean | null;

export interface CaptureEntry {
  value: CapturedValue;
  /** True when the key matched the secret-name regex
   * (`password|token|secret|key|auth*`). */
  isSecret: boolean;
}

export type ByAlias = Readonly<
  Record<string, Readonly<Record<string, CaptureEntry>>>
>;
export type ByFile = Readonly<Record<string, ByAlias>>;

interface CaptureState {
  values: ByFile;
  /** Replace every capture for a given (file, alias) — typically
   * called after a successful run with the freshly evaluated map. */
  setBlockCaptures: (
    filePath: string,
    alias: string,
    captures: Readonly<Record<string, unknown>>,
  ) => void;
  /** Drop every capture for a single block. */
  clearBlockCaptures: (filePath: string, alias: string) => void;
  /** Drop every capture for an entire file (e.g. on file close). */
  clearFile: (filePath: string) => void;
  /** Reset the whole store (e.g. on app restart — also achieved by
   * not persisting). */
  clearAll: () => void;
  /** Read accessor — undefined when not present. */
  getCapture: (
    filePath: string,
    alias: string,
    key: string,
  ) => CaptureEntry | undefined;
  /** Read accessor — every capture for one block (or empty object). */
  getBlockCaptures: (
    filePath: string,
    alias: string,
  ) => Readonly<Record<string, CaptureEntry>>;
  /** Hydrate a file's captures from the JSON shape persisted in
   * `.httui/captures/<file_relpath>.json`. Tolerant: invalid JSON or
   * an unexpected shape is a no-op (the cache is best-effort). The
   * `isSecret` flag is re-derived from the key name on insert, so
   * the persisted shape doesn't need to round-trip it. */
  loadFromCacheJson: (filePath: string, json: string) => void;
  /** Serialize a file's captures into the persistence shape, dropping
   * every entry whose key matched the secret-name regex. Returns
   * `null` when there's nothing to persist (consumer should skip the
   * write). */
  dumpForCacheJson: (filePath: string) => string | null;
}

export const useCaptureStore = create<CaptureState>()(
  devtools(
    (set, get) => ({
      values: {},

      setBlockCaptures: (filePath, alias, raw) =>
        set(
          (state) => {
            const block: Record<string, CaptureEntry> = {};
            for (const [k, v] of Object.entries(raw)) {
              block[k] = {
                value: coerceCapturedValue(v),
                isSecret: isSecretCaptureKey(k),
              };
            }
            return {
              values: {
                ...state.values,
                [filePath]: {
                  ...(state.values[filePath] ?? {}),
                  [alias]: block,
                },
              },
            };
          },
          false,
          "captures/set",
        ),

      clearBlockCaptures: (filePath, alias) =>
        set(
          (state) => {
            const fileMap = state.values[filePath];
            if (!fileMap || !(alias in fileMap)) return state;
            const nextFile = filterMap(fileMap, alias);
            const nextValues = { ...state.values };
            if (Object.keys(nextFile).length === 0) {
              delete nextValues[filePath];
            } else {
              nextValues[filePath] = nextFile;
            }
            return { values: nextValues };
          },
          false,
          "captures/clearBlock",
        ),

      clearFile: (filePath) =>
        set(
          (state) => {
            if (!(filePath in state.values)) return state;
            const next = { ...state.values };
            delete next[filePath];
            return { values: next };
          },
          false,
          "captures/clearFile",
        ),

      clearAll: () => set({ values: {} }, false, "captures/clearAll"),

      getCapture: (filePath, alias, key) =>
        get().values[filePath]?.[alias]?.[key],

      getBlockCaptures: (filePath, alias) =>
        get().values[filePath]?.[alias] ?? {},

      loadFromCacheJson: (filePath, json) => {
        const parsed = parseCacheJson(json);
        if (!parsed) return;
        set(
          (state) => {
            const fileMap: Record<string, Record<string, CaptureEntry>> = {};
            for (const [alias, byKey] of Object.entries(parsed)) {
              const block: Record<string, CaptureEntry> = {};
              for (const [k, v] of Object.entries(byKey)) {
                block[k] = {
                  value: coerceCapturedValue(v),
                  isSecret: isSecretCaptureKey(k),
                };
              }
              fileMap[alias] = block;
            }
            return {
              values: { ...state.values, [filePath]: fileMap },
            };
          },
          false,
          "captures/loadFromCache",
        );
      },

      dumpForCacheJson: (filePath) => {
        const fileMap = get().values[filePath];
        if (!fileMap) return null;
        const out: Record<string, Record<string, CapturedValue>> = {};
        for (const [alias, byKey] of Object.entries(fileMap)) {
          const block: Record<string, CapturedValue> = {};
          for (const [k, entry] of Object.entries(byKey)) {
            if (entry.isSecret) continue;
            block[k] = entry.value;
          }
          if (Object.keys(block).length > 0) {
            out[alias] = block;
          }
        }
        if (Object.keys(out).length === 0) return null;
        return JSON.stringify(out);
      },
    }),
    { name: "capture-store" },
  ),
);

/** Parse + shape-validate the persisted JSON. Returns the parsed
 * `{ alias: { key: value } }` map, or `null` when the input is
 * unparseable / not the expected shape. The `value` is left as
 * `unknown` so `coerceCapturedValue` can apply the same primitives-
 * only rule used at runtime. */
function parseCacheJson(
  json: string,
): Record<string, Record<string, unknown>> | null {
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return null;
  }
  if (!isPlainObject(raw)) return null;
  const out: Record<string, Record<string, unknown>> = {};
  for (const [alias, byKey] of Object.entries(raw)) {
    if (!isPlainObject(byKey)) continue;
    out[alias] = byKey;
  }
  return out;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function filterMap<T>(
  obj: Readonly<Record<string, T>>,
  drop: string,
): Record<string, T> {
  const out: Record<string, T> = {};
  for (const [k, v] of Object.entries(obj)) {
    if (k !== drop) out[k] = v;
  }
  return out;
}

/** Coerce captured value into the persisted shape. Strings, numbers,
 * booleans, null pass through; objects/arrays/undefined collapse to
 * `null` (the consumer typically renders `(empty)` and shows the path
 * as a hint). */
function coerceCapturedValue(v: unknown): CapturedValue {
  if (v === null) return null;
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return v;
  return null;
}
