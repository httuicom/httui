import { invoke } from "@tauri-apps/api/core";

export interface Connection {
  id: string;
  name: string;
  driver: "postgres" | "mysql" | "sqlite";
  host: string | null;
  port: number | null;
  database_name: string | null;
  username: string | null;
  has_password: boolean;
  ssl_mode: string | null;
  timeout_ms: number;
  query_timeout_ms: number;
  ttl_seconds: number;
  max_pool_size: number;
  /** When true, the frontend confirms every mutation before sending it. */
  is_readonly: boolean;
  last_tested_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateConnectionInput {
  name: string;
  driver: "postgres" | "mysql" | "sqlite";
  host?: string;
  port?: number;
  database_name?: string;
  username?: string;
  password?: string;
  ssl_mode?: string;
  timeout_ms?: number;
  query_timeout_ms?: number;
  ttl_seconds?: number;
  max_pool_size?: number;
  is_readonly?: boolean;
}

export interface UpdateConnectionInput {
  name?: string;
  driver?: string;
  host?: string;
  port?: number;
  database_name?: string;
  username?: string;
  password?: string;
  ssl_mode?: string;
  timeout_ms?: number;
  query_timeout_ms?: number;
  ttl_seconds?: number;
  max_pool_size?: number;
  is_readonly?: boolean;
}

export function listConnections(): Promise<Connection[]> {
  return invoke("list_connections");
}

export function createConnection(
  input: CreateConnectionInput,
): Promise<Connection> {
  return invoke("create_connection", { input });
}

export function updateConnection(
  id: string,
  input: UpdateConnectionInput,
): Promise<Connection> {
  return invoke("update_connection", { id, input });
}

export function deleteConnection(id: string): Promise<void> {
  return invoke("delete_connection", { id });
}

export function testConnection(id: string): Promise<void> {
  return invoke("test_connection", { id });
}

/** Vault-grep for db-block fences using `connection=<name>`. Powers
 * the "Used in runbooks" panel of ConnectionsPage. */
export interface ConnectionUse {
  /** Vault-relative path with posix separators. */
  file: string;
  /** 1-based line number of the fence opener. */
  line: number;
}

export function findConnectionUses(
  vaultPath: string,
  connectionName: string,
): Promise<ConnectionUse[]> {
  return invoke("find_connection_uses_cmd", { vaultPath, connectionName });
}

export interface SchemaEntry {
  /** Null for SQLite; the namespace for Postgres/MySQL (`public`, `vendas`, …). */
  schema_name: string | null;
  table_name: string;
  column_name: string;
  data_type: string | null;
}

export function introspectSchema(connectionId: string): Promise<SchemaEntry[]> {
  return invoke("introspect_schema", { connectionId });
}

export function getCachedSchema(
  connectionId: string,
  ttlSeconds?: number,
): Promise<SchemaEntry[] | null> {
  return invoke("get_cached_schema", { connectionId, ttlSeconds });
}
