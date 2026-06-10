// Shared fixtures for the db fenced panel + sibling hook tests.
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import type { DbPortalEntry } from "@/lib/codemirror/cm-db-block";
import type { DbBlockMetadata } from "@/lib/blocks/db-fence";
import type { Connection } from "@/lib/tauri/connections";
import type { DbResponse, DbRow } from "@/components/blocks/db/types";

export function makeView(doc: string): EditorView {
  const container = document.createElement("div");
  document.body.appendChild(container);
  return new EditorView({
    state: EditorState.create({ doc }),
    parent: container,
  });
}

/** Builds a db block + view whose doc is "```db-postgres …\n<body>\n```". */
export function makeDbBlock(
  view: EditorView,
  meta: Partial<DbBlockMetadata> = {},
  body = "SELECT 1;",
): DbPortalEntry["block"] {
  const infoTokens = Object.entries({
    alias: meta.alias,
    connection: meta.connection,
  })
    .filter(([, v]) => v !== undefined)
    .map(([k, v]) => ` ${k}=${v}`)
    .join("");
  const open = "```db-postgres" + infoTokens;
  const docText = `${open}\n${body}\n\`\`\``;
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: docText },
  });
  const openLineFrom = 0;
  const openLineTo = open.length;
  const bodyFrom = openLineTo + 1;
  const bodyTo = bodyFrom + body.length;
  const closeLineFrom = bodyTo + 1;
  const closeLineTo = closeLineFrom + 3;
  return {
    from: 0,
    to: closeLineTo,
    info: infoTokens.trim(),
    lang: "db-postgres",
    openLineFrom,
    openLineTo,
    bodyFrom,
    bodyTo,
    closeLineFrom,
    closeLineTo,
    body,
    metadata: { dialect: "postgres", ...meta },
  };
}

export function makeConnection(over: Partial<Connection> = {}): Connection {
  return {
    id: "c1",
    name: "local",
    driver: "postgres",
    host: "localhost",
    port: 5432,
    database_name: "db",
    username: "u",
    has_password: false,
    ssl_mode: null,
    timeout_ms: 30000,
    query_timeout_ms: 30000,
    ttl_seconds: 300,
    max_pool_size: 5,
    is_readonly: false,
    last_tested_at: null,
    created_at: "",
    updated_at: "",
    ...over,
  };
}

export function selectResponse(
  rows: DbRow[],
  opts: { hasMore?: boolean; elapsedMs?: number } = {},
): DbResponse {
  const names = rows.length > 0 ? Object.keys(rows[0]) : ["x"];
  return {
    results: [
      {
        kind: "select",
        columns: names.map((name) => ({ name, type: "text" })),
        rows,
        has_more: opts.hasMore ?? false,
      },
    ],
    messages: [],
    stats: { elapsed_ms: opts.elapsedMs ?? 7 },
  };
}
