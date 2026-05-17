// Session-scoped connection host:port overrides (V11 cenário 2).
//
// In-memory only. An override is TEMPORARY: it wins over the
// vault-stored host/port for the session and disappears when the app
// restarts. Never persisted to disk. The value is passed per DB run
// through `executeDbStreamed` → backend `get_pool_with_override`,
// which creates an override-keyed pool without touching the base one.
//
// Mirrors the env-var `sessionOverride` store's shape/conventions, but
// keyed by connection id with a `{ host?, port? }` pair per connection.

import { create } from "zustand";
import { devtools } from "zustand/middleware";

export interface ConnectionOverride {
  host?: string;
  port?: number;
}

export type ConnectionOverrides = Readonly<
  Record<string, Readonly<ConnectionOverride>>
>;

interface ConnectionSessionOverrideState {
  overrides: ConnectionOverrides;
  /** Merge a host/port patch for `connectionId`. Empty patch (both
   * undefined/blank) drops the override entirely. */
  setOverride: (connectionId: string, patch: ConnectionOverride) => void;
  /** Drop the override for `connectionId`. No-op if not set. */
  clearOverride: (connectionId: string) => void;
  /** Reset every override. */
  clearAll: () => void;
  /** Read accessor — undefined when no override is set. */
  getOverride: (connectionId: string) => ConnectionOverride | undefined;
}

function normalize(patch: ConnectionOverride): ConnectionOverride | null {
  const host =
    typeof patch.host === "string" && patch.host.trim() !== ""
      ? patch.host.trim()
      : undefined;
  const port =
    typeof patch.port === "number" && Number.isFinite(patch.port)
      ? patch.port
      : undefined;
  if (host === undefined && port === undefined) return null;
  const out: ConnectionOverride = {};
  if (host !== undefined) out.host = host;
  if (port !== undefined) out.port = port;
  return out;
}

export const useConnectionSessionOverrideStore =
  create<ConnectionSessionOverrideState>()(
    devtools(
      (set, get) => ({
        overrides: {},

        setOverride: (connectionId, patch) =>
          set(
            (state) => {
              const next = normalize(patch);
              const map = { ...state.overrides };
              if (next === null) {
                delete map[connectionId];
              } else {
                map[connectionId] = next;
              }
              return { overrides: map };
            },
            false,
            "connOverride/set",
          ),

        clearOverride: (connectionId) =>
          set(
            (state) => {
              if (!(connectionId in state.overrides)) return state;
              const map = { ...state.overrides };
              delete map[connectionId];
              return { overrides: map };
            },
            false,
            "connOverride/clear",
          ),

        clearAll: () => set({ overrides: {} }, false, "connOverride/clearAll"),

        getOverride: (connectionId) => get().overrides[connectionId],
      }),
      { name: "connection-session-override-store" },
    ),
  );
