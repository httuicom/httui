// Canvas §5 connection-kind metadata — Epic 42 Story 01.
//
// Single source of truth for the 9 supported kinds: name, lucide
// icon component, accent hue. Consumed by `<ConnectionKindIcon>`,
// `<ConnectionKindFilter>` (sidebar), and the list-row icon column.
//
// Icons via react-icons/lu — feedback_no_emojis_use_icons.

import type { IconType } from "react-icons";
import {
  LuArrowLeftRight,
  LuChartBar,
  LuDatabase,
  LuDatabaseZap,
  LuDiamond,
  LuFileText,
  LuGlobe,
  LuLeaf,
  LuTerminal,
  LuZap,
} from "react-icons/lu";

export type ConnectionKind =
  | "postgres"
  | "mysql"
  | "sqlite"
  | "mongo"
  | "bigquery"
  | "grpc"
  | "graphql"
  | "http"
  | "ws"
  | "shell";

export interface ConnectionKindMeta {
  kind: ConnectionKind;
  label: string;
  Icon: IconType;
  /** oklch lightness/chroma/hue triple (no `oklch()` wrapper). */
  hue: string;
}

export const CONNECTION_KINDS: Readonly<
  Record<ConnectionKind, ConnectionKindMeta>
> = {
  postgres: {
    kind: "postgres",
    label: "PostgreSQL",
    Icon: LuDatabase,
    hue: "0.62 0.10 250",
  },
  mysql: {
    kind: "mysql",
    label: "MySQL / MariaDB",
    Icon: LuDatabaseZap,
    hue: "0.62 0.10 215",
  },
  sqlite: {
    kind: "sqlite",
    label: "SQLite",
    Icon: LuFileText,
    hue: "0.62 0.10 200",
  },
  mongo: {
    kind: "mongo",
    label: "MongoDB",
    Icon: LuLeaf,
    hue: "0.55 0.13 145",
  },
  bigquery: {
    kind: "bigquery",
    label: "BigQuery",
    Icon: LuChartBar,
    hue: "0.62 0.10 240",
  },
  grpc: {
    kind: "grpc",
    label: "gRPC",
    Icon: LuZap,
    hue: "0.62 0.14 280",
  },
  graphql: {
    kind: "graphql",
    label: "GraphQL",
    Icon: LuDiamond,
    hue: "0.62 0.16 330",
  },
  http: {
    kind: "http",
    label: "HTTP / REST base URL",
    Icon: LuGlobe,
    hue: "0.74 0.07 215",
  },
  ws: {
    kind: "ws",
    label: "WebSocket",
    Icon: LuArrowLeftRight,
    hue: "0.62 0.10 215",
  },
  shell: {
    kind: "shell",
    label: "Shell / Bash",
    Icon: LuTerminal,
    hue: "0.50 0.014 240",
  },
};

/** Stable display order for the sidebar filter list. */
export const CONNECTION_KIND_ORDER: ReadonlyArray<ConnectionKind> = [
  "postgres",
  "mysql",
  "sqlite",
  "mongo",
  "bigquery",
  "grpc",
  "graphql",
  "http",
  "ws",
  "shell",
];

/** Drivers that the V1 NewConnectionModal can actually create. Other
 * kinds surface a "Coming soon" empty state in the modal body and
 * hide the Form/Connection-string/SSH/SSL tabs. */
export const SUPPORTED_NEW_CONNECTION_KINDS: ReadonlyArray<ConnectionKind> = [
  "postgres",
  "mysql",
  "sqlite",
];

/** `oklch(hue)` wrapper — convenience for inline style consumers. */
export function kindColor(kind: ConnectionKind): string {
  return `oklch(${CONNECTION_KINDS[kind].hue})`;
}

/** Map the legacy `Connection.driver` (postgres / mysql / sqlite) to
 * the canvas-§5 `ConnectionKind`. */
export function kindFromDriver(driver: string): ConnectionKind | null {
  switch (driver) {
    case "postgres":
      return "postgres";
    case "mysql":
      return "mysql";
    case "sqlite":
      return "sqlite";
    default:
      return null;
  }
}
