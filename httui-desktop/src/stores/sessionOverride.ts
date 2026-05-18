// Session-scoped variable overrides.
//
// In-memory only. Overrides are TEMPORARY: they win over the
// vault-stored value during the session and disappear when the app
// restarts. Never persisted to disk; never sent through any Tauri
// command. Domain mirror of `Map<env, Map<key, value>>` using plain
// records so React/Zustand reactivity works without manual replace.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

export type SessionOverrides = Readonly<
  Record<string, Readonly<Record<string, string>>>
>;

interface SessionOverrideState {
  overrides: SessionOverrides;
  /** Add/replace an override for `env` × `key`. */
  setOverride: (env: string, key: string, value: string) => void;
  /** Drop the override for `env` × `key`. No-op if not set. */
  clearOverride: (env: string, key: string) => void;
  /** Drop every override for the given key (used on variable rename/delete). */
  clearAllForKey: (key: string) => void;
  /** Reset all overrides at once. */
  clearAll: () => void;
  /** Read accessor — returns undefined when no override is set. */
  getOverride: (env: string, key: string) => string | undefined;
}

function filterEnvMap(
  envMap: Readonly<Record<string, string>>,
  dropKey: string,
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(envMap)) {
    if (k !== dropKey) out[k] = v;
  }
  return out;
}

export const useSessionOverrideStore = create<SessionOverrideState>()(
  devtools(
    (set, get) => ({
      overrides: {},

      setOverride: (env, key, value) =>
        set(
          (state) => ({
            overrides: {
              ...state.overrides,
              [env]: { ...(state.overrides[env] ?? {}), [key]: value },
            },
          }),
          false,
          "sessionOverride/set",
        ),

      clearOverride: (env, key) =>
        set(
          (state) => {
            const envMap = state.overrides[env];
            if (!envMap || !(key in envMap)) return state;
            const rest = filterEnvMap(envMap, key);
            const next = { ...state.overrides };
            if (Object.keys(rest).length === 0) {
              delete next[env];
            } else {
              next[env] = rest;
            }
            return { overrides: next };
          },
          false,
          "sessionOverride/clear",
        ),

      clearAllForKey: (key) =>
        set(
          (state) => {
            const next: Record<string, Record<string, string>> = {};
            let changed = false;
            for (const [env, envMap] of Object.entries(state.overrides)) {
              if (!(key in envMap)) {
                next[env] = envMap;
                continue;
              }
              changed = true;
              const rest = filterEnvMap(envMap, key);
              if (Object.keys(rest).length > 0) next[env] = rest;
            }
            return changed ? { overrides: next } : state;
          },
          false,
          "sessionOverride/clearAllForKey",
        ),

      clearAll: () => set({ overrides: {} }, false, "sessionOverride/clearAll"),

      getOverride: (env, key) => get().overrides[env]?.[key],
    }),
    { name: "session-override-store" },
  ),
);
