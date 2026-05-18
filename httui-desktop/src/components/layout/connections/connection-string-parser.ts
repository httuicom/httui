// Canvas §5 — Connection-string parser.
//
// Pure helper for the "Connection string" tab in the Nova Conexão
// modal. Accepts a URL the user pasted (postgres / postgresql / mysql
// for v1; other kinds reject explicitly until we ship per-kind shapes)
// and returns a discriminated result with the form fields + a parsed
// SSL payload extracted from query params.

import type { ConnectionKind } from "./connection-kinds";
import {
  EMPTY_POSTGRES_VALUE,
  type PostgresFormValue,
} from "./NewConnectionFormTab";
import {
  EMPTY_SSL_VALUE,
  isSslMode,
  type SslFormValue,
  type SslMode,
} from "./NewConnectionSslTab";

export type ConnectionStringParseResult =
  | {
      ok: true;
      kind: ConnectionKind;
      value: PostgresFormValue;
      ssl: SslFormValue;
    }
  | { ok: false; error: string };

const SCHEME_TO_KIND: Record<string, ConnectionKind> = {
  postgres: "postgres",
  postgresql: "postgres",
  mysql: "mysql",
  mariadb: "mysql",
};

const DEFAULT_PORT: Record<ConnectionKind, string> = {
  postgres: "5432",
  mysql: "3306",
  sqlite: "",
  mongo: "27017",
  bigquery: "",
  grpc: "",
  graphql: "",
  http: "",
  ws: "",
  shell: "",
};

export function parseConnectionString(
  input: string,
): ConnectionStringParseResult {
  const trimmed = input.trim();
  if (!trimmed) {
    return { ok: false, error: "Paste a connection string to begin." };
  }

  const schemeMatch = /^([a-zA-Z][a-zA-Z0-9+.-]*):\/\//.exec(trimmed);
  if (!schemeMatch) {
    return {
      ok: false,
      error: "Connection string precisa começar com `<driver>://`.",
    };
  }

  const scheme = schemeMatch[1].toLowerCase();
  const kind = SCHEME_TO_KIND[scheme];
  if (!kind) {
    return {
      ok: false,
      error: `Driver "${scheme}" ainda não suportado neste assistente. Use Form ou cole postgres/mysql.`,
    };
  }

  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return {
      ok: false,
      error: "URL inválida. Verifique host, porta e caracteres especiais.",
    };
  }

  const username = url.username ? decodeURIComponent(url.username) : "";
  const password = url.password ? decodeURIComponent(url.password) : "";
  const host = url.hostname || "";
  const port = url.port || DEFAULT_PORT[kind];
  const database = url.pathname.replace(/^\/+/, "");

  const value: PostgresFormValue = {
    ...EMPTY_POSTGRES_VALUE,
    name: database || host,
    host,
    port,
    database,
    username,
    password,
  };

  const ssl = extractSsl(url);

  return { ok: true, kind, value, ssl };
}

function extractSsl(url: URL): SslFormValue {
  const ssl: SslFormValue = { ...EMPTY_SSL_VALUE };
  const params = url.searchParams;

  const mode = params.get("sslmode");
  if (mode && isSslMode(mode)) {
    ssl.mode = mode as SslMode;
  } else if (params.get("ssl") === "true" || params.get("ssl") === "1") {
    ssl.mode = "require";
  }

  const rootCert = params.get("sslrootcert") ?? params.get("ssl-ca");
  if (rootCert) ssl.rootCertPath = rootCert;

  const clientCert = params.get("sslcert") ?? params.get("ssl-cert");
  if (clientCert) ssl.clientCertPath = clientCert;

  const clientKey = params.get("sslkey") ?? params.get("ssl-key");
  if (clientKey) ssl.clientKeyPath = clientKey;

  return ssl;
}
