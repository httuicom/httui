// "Form" tab for the new-connection modal.
// Postgres/MySQL: full field grid. SQLite: file path + browse.
// Other kinds: "coming soon" stub. Pure presentational — value lifted to consumer.

import {
  Box,
  Flex,
  Grid,
  HStack,
  IconButton,
  Text,
  chakra,
} from "@chakra-ui/react";
import type { ReactNode } from "react";
import { LuFolderOpen, LuKey } from "react-icons/lu";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import { Input } from "@/components/atoms";

import type { ConnectionKind } from "./connection-kinds";

const KEYCHAIN_HINT =
  "Saved only in your local keychain. New device → re-register.";

/** Field set for the Postgres-shape form (also used by MySQL/SQLite subsets). */
export interface PostgresFormValue {
  name: string;
  host: string;
  port: string;
  database: string;
  username: string;
  password: string;
}

export const EMPTY_POSTGRES_VALUE: PostgresFormValue = {
  name: "",
  host: "localhost",
  port: "5432",
  database: "",
  username: "",
  password: "",
};

/** Per-kind form defaults (port, host) for when the user switches drivers. */
export function emptyFormValueForKind(kind: ConnectionKind): PostgresFormValue {
  switch (kind) {
    case "mysql":
      return { ...EMPTY_POSTGRES_VALUE, port: "3306" };
    case "sqlite":
      return { ...EMPTY_POSTGRES_VALUE, host: "", port: "" };
    default:
      return EMPTY_POSTGRES_VALUE;
  }
}

export interface NewConnectionFormTabProps {
  kind: ConnectionKind;
  value: PostgresFormValue;
  onChange: (next: PostgresFormValue) => void;
  envBinder?: ReactNode;
  testBanner?: ReactNode;
}

export function NewConnectionFormTab({
  kind,
  value,
  onChange,
  envBinder,
  testBanner,
}: NewConnectionFormTabProps) {
  if (kind === "sqlite") {
    return (
      <Flex
        data-testid="new-connection-form-tab-sqlite"
        direction="column"
        gap={4}
      >
        <Field label="Name">
          <Input
            data-testid="new-connection-field-name"
            value={value.name}
            onChange={(e) => patch("name", e.target.value)}
            placeholder="local-cache"
          />
        </Field>
        <Field
          label="Database file path"
          hint="Absolute or vault-relative path to the .sqlite file."
        >
          <HStack gap={2} align="center">
            <Input
              data-testid="new-connection-field-database"
              value={value.database}
              onChange={(e) => patch("database", e.target.value)}
              placeholder="~/data/cache.sqlite"
              flex={1}
            />
            <IconButton
              data-testid="new-connection-field-database-browse"
              aria-label="Browse for database file"
              title="Browse…"
              variant="ghost"
              size="sm"
              onClick={async () => {
                try {
                  const picked = await openFileDialog({
                    multiple: false,
                    directory: false,
                    title: "Select SQLite database file",
                    filters: [
                      {
                        name: "SQLite",
                        extensions: ["sqlite", "sqlite3", "db"],
                      },
                      { name: "All files", extensions: ["*"] },
                    ],
                  });
                  if (typeof picked === "string" && picked.length > 0) {
                    patch("database", picked);
                  }
                } catch {
                  // ignore — user dismissed or dialog plugin unavailable
                }
              }}
            >
              <LuFolderOpen />
            </IconButton>
          </HStack>
        </Field>
        {envBinder && (
          <Box data-testid="new-connection-form-env-slot">{envBinder}</Box>
        )}
        {testBanner && (
          <Box data-testid="new-connection-form-test-slot">{testBanner}</Box>
        )}
      </Flex>
    );
  }

  if (kind !== "postgres" && kind !== "mysql") {
    return (
      <Box
        data-testid={`new-connection-form-stub-${kind}`}
        fontSize="12px"
        color="fg.subtle"
      >
        Form for “{kind}” coming in a future phase.
      </Box>
    );
  }

  function patch<K extends keyof PostgresFormValue>(
    field: K,
    next: PostgresFormValue[K],
  ) {
    onChange({ ...value, [field]: next });
  }

  return (
    <Flex data-testid="new-connection-form-tab" direction="column" gap={4}>
      <Field label="Name">
        <Input
          data-testid="new-connection-field-name"
          value={value.name}
          onChange={(e) => patch("name", e.target.value)}
          placeholder="prod-orders-rw"
        />
      </Field>

      <Grid gridTemplateColumns="2fr 90px" gap={3}>
        <Field label="Host">
          <Input
            data-testid="new-connection-field-host"
            value={value.host}
            onChange={(e) => patch("host", e.target.value)}
            placeholder="db.internal"
          />
        </Field>
        <Field label="Port">
          <Input
            data-testid="new-connection-field-port"
            value={value.port}
            onChange={(e) => patch("port", e.target.value)}
            inputMode="numeric"
            placeholder={kind === "mysql" ? "3306" : "5432"}
          />
        </Field>
      </Grid>

      <Grid gridTemplateColumns="1fr 1fr" gap={3}>
        <Field label="Database">
          <Input
            data-testid="new-connection-field-database"
            value={value.database}
            onChange={(e) => patch("database", e.target.value)}
            placeholder="orders"
          />
        </Field>
        <Field label="User">
          <Input
            data-testid="new-connection-field-username"
            value={value.username}
            onChange={(e) => patch("username", e.target.value)}
            placeholder="orders_app"
          />
        </Field>
      </Grid>

      <Field label="Password" hint={KEYCHAIN_HINT}>
        <Flex align="center" gap={2}>
          <Input
            data-testid="new-connection-field-password"
            type="password"
            value={value.password}
            onChange={(e) => patch("password", e.target.value)}
            placeholder="••••••••"
            flex={1}
          />
          <KeychainSuffix />
        </Flex>
      </Field>

      {envBinder && (
        <Box data-testid="new-connection-form-env-slot">{envBinder}</Box>
      )}

      {testBanner && (
        <Box data-testid="new-connection-form-test-slot">{testBanner}</Box>
      )}
    </Flex>
  );
}

const FieldRoot = chakra("label");

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <FieldRoot
      display="flex"
      flexDirection="column"
      gap={1}
      data-testid={`new-connection-form-field-${label.toLowerCase()}`}
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        fontWeight="bold"
        letterSpacing="0.06em"
        textTransform="uppercase"
        color="fg.muted"
      >
        {label}
      </Text>
      {children}
      {hint && (
        <Text as="span" fontSize="11px" color="fg.subtle">
          {hint}
        </Text>
      )}
    </FieldRoot>
  );
}

function KeychainSuffix() {
  return (
    <HStack
      data-testid="new-connection-keychain-suffix"
      flexShrink={0}
      fontSize="10px"
      color="fg.muted"
      bg="bg.muted"
      borderWidth="1px"
      borderColor="border"
      borderRadius="999px"
      px="8px"
      py="2px"
      gap={1}
    >
      <LuKey size={10} />
      <Text as="span">keychain</Text>
    </HStack>
  );
}
