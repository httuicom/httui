// ConnectionForm state — extracted to kill the 18-useState sprawl and
// the props→state driver→port mirror effect (audit 02 §4 / 05 Part B,
// backlog F4). Pure: reducer + initializer + a first-error validator,
// unit-tested in isolation alongside the validateEnvName / variable-
// name.ts convention.

import type {
  Connection,
  CreateConnectionInput,
} from "@/lib/tauri/connections";

import { DRIVER_CONFIG, type Driver } from "./form/DriverSelector";

/** Every text field driven by an `<Input>` / select in the form. The
 *  generic `setField` action keys on these so the component never
 *  hand-rolls a setter per field (the desync source). */
export type TextField =
  | "name"
  | "host"
  | "port"
  | "dbName"
  | "username"
  | "password"
  | "sslMode"
  | "timeoutMs"
  | "queryTimeoutMs"
  | "ttlSeconds"
  | "maxPoolSize";

export interface ConnectionFormState {
  name: string;
  driver: Driver;
  host: string;
  port: string;
  dbName: string;
  username: string;
  password: string;
  sslMode: string;
  showAdvanced: boolean;
  timeoutMs: string;
  queryTimeoutMs: string;
  ttlSeconds: string;
  maxPoolSize: string;
  saving: boolean;
  testing: boolean;
  testResult: "success" | "error" | null;
  testError: string | null;
  error: string | null;
}

const TRANSIENT = {
  showAdvanced: false,
  saving: false,
  testing: false,
  testResult: null,
  testError: null,
  error: null,
} as const;

/** The new-connection defaults. Port is the postgres default —
 *  exactly what the old mount `useEffect([driver,isEdit])` set, so
 *  dropping that effect (the props→state smell) is behavior-
 *  preserving. Zero branches: split out of `initConnectionFormState`
 *  so neither function trips the cyclomatic-complexity rule. */
export function emptyConnectionFormState(): ConnectionFormState {
  return {
    name: "",
    driver: "postgres",
    host: "localhost",
    port: DRIVER_CONFIG.postgres.defaultPort,
    dbName: "",
    username: "",
    password: "",
    sslMode: "disable",
    timeoutMs: "10000",
    queryTimeoutMs: "30000",
    ttlSeconds: "300",
    maxPoolSize: "5",
    ...TRANSIENT,
  };
}

/**
 * Replaces the 18 `useState(connection?…)` initializers verbatim. New
 * → driver defaults; edit → prefilled from the stored row, keeping the
 * stored port (the old effect was gated on `!isEdit`) and never
 * echoing the password back.
 */
export function initConnectionFormState(
  connection: Connection | null,
): ConnectionFormState {
  if (!connection) return emptyConnectionFormState();
  return {
    name: connection.name ?? "",
    driver: (connection.driver as Driver) ?? "postgres",
    host: connection.host ?? "localhost",
    port: connection.port?.toString() ?? "5432",
    dbName: connection.database_name ?? "",
    username: connection.username ?? "",
    password: "",
    sslMode: connection.ssl_mode ?? "disable",
    timeoutMs: (connection.timeout_ms ?? 10000).toString(),
    queryTimeoutMs: (connection.query_timeout_ms ?? 30000).toString(),
    ttlSeconds: (connection.ttl_seconds ?? 300).toString(),
    maxPoolSize: (connection.max_pool_size ?? 5).toString(),
    ...TRANSIENT,
  };
}

export type ConnectionFormAction =
  | { type: "setField"; field: TextField; value: string }
  | { type: "setDriver"; driver: Driver; isEdit: boolean }
  | { type: "toggleAdvanced" }
  | { type: "saveStart" }
  | { type: "saveError"; message: string }
  | { type: "saveDone" }
  | { type: "testStart" }
  | { type: "testSuccess" }
  | { type: "testFailure"; message: string };

export function connectionFormReducer(
  state: ConnectionFormState,
  action: ConnectionFormAction,
): ConnectionFormState {
  switch (action.type) {
    case "setField":
      return { ...state, [action.field]: action.value };
    case "setDriver":
      // Driver swap re-derives the default port — but only for a new
      // connection (editing keeps the stored port). This is the old
      // `useEffect([driver,isEdit])` made synchronous + desync-free.
      return {
        ...state,
        driver: action.driver,
        port: action.isEdit
          ? state.port
          : DRIVER_CONFIG[action.driver].defaultPort,
      };
    case "toggleAdvanced":
      return { ...state, showAdvanced: !state.showAdvanced };
    case "saveStart":
      return { ...state, error: null, saving: true };
    case "saveError":
      return { ...state, error: action.message, saving: false };
    case "saveDone":
      return { ...state, saving: false };
    case "testStart":
      return { ...state, testing: true, testResult: null, testError: null };
    case "testSuccess":
      return { ...state, testing: false, testResult: "success" };
    case "testFailure":
      return {
        ...state,
        testing: false,
        testResult: "error",
        testError: action.message,
      };
  }
}

export type ConnectionValidation = { ok: true } | { ok: false; reason: string };

/**
 * The field validation the form never had (audit 05 Part B: "No field
 * validation at all"). Returns the *first* problem so it can surface
 * in the existing single error Badge — no per-field error map, because
 * the form has no per-field error UI (adding one would be
 * over-engineering). Mirrors the `{ok}|{ok:false,reason}` shape of
 * `validateEnvName` / `validateVariableName`.
 */
export function validateConnection(
  state: ConnectionFormState,
): ConnectionValidation {
  if (!state.name.trim()) {
    return { ok: false, reason: "Connection name is required" };
  }
  if (state.driver === "sqlite") {
    if (!state.dbName.trim()) {
      return { ok: false, reason: "SQLite file path is required" };
    }
    return { ok: true };
  }
  if (!state.host.trim()) {
    return { ok: false, reason: "Host is required" };
  }
  const port = Number(state.port);
  if (
    !state.port.trim() ||
    !Number.isInteger(port) ||
    port <= 0 ||
    port > 65535
  ) {
    return { ok: false, reason: "Port must be a number between 1 and 65535" };
  }
  return { ok: true };
}

/**
 * Map the form state to the IPC payload. Extracted from the
 * component's `handleSave` (where the driver-conditional spreads
 * inflated its cyclomatic complexity) so it is pure + unit-testable.
 * `parseInt(...) || undefined` keeps the prior lenient numeric
 * coercion verbatim — `validateConnection` is the real gate.
 */
export function buildConnectionInput(
  state: ConnectionFormState,
): CreateConnectionInput {
  const network = state.driver !== "sqlite";
  return {
    name: state.name,
    driver: state.driver,
    ...(network && {
      host: state.host,
      port: parseInt(state.port) || undefined,
      username: state.username || undefined,
      password: state.password || undefined,
      ssl_mode: state.sslMode,
    }),
    database_name: state.dbName || undefined,
    timeout_ms: parseInt(state.timeoutMs) || undefined,
    query_timeout_ms: parseInt(state.queryTimeoutMs) || undefined,
    ttl_seconds: parseInt(state.ttlSeconds) || undefined,
    max_pool_size: parseInt(state.maxPoolSize) || undefined,
  };
}
