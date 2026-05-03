// Canvas §5 — "SSL" tab for the Nova Conexão modal
// (Epic 42 Story 06 — Phase 3).
//
// Renders the SSL configuration fields: sslmode select + root cert
// path + client cert + client key paths. Pure presentational; the
// consumer owns the value and onChange. Used both as a tab and as
// the secondary patch surface when a connection string carries
// ?sslmode=...&sslrootcert=... params.

import { Box, Flex, Grid, HStack, IconButton, Text, chakra } from "@chakra-ui/react";
import type { ReactNode } from "react";
import { LuFolderOpen } from "react-icons/lu";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import { Input } from "@/components/atoms";

export const SSL_MODES = [
  "",
  "disable",
  "allow",
  "prefer",
  "require",
  "verify-ca",
  "verify-full",
] as const;

export type SslMode = (typeof SSL_MODES)[number];

export function isSslMode(value: string): value is SslMode {
  return (SSL_MODES as readonly string[]).includes(value);
}

const SSL_MODE_LABEL: Record<SslMode, string> = {
  "": "(driver default)",
  disable: "disable — never use SSL",
  allow: "allow — fall back to plain",
  prefer: "prefer — try SSL first",
  require: "require — must use SSL",
  "verify-ca": "verify-ca — check chain",
  "verify-full": "verify-full — check host",
};

export interface SslFormValue {
  mode: SslMode;
  rootCertPath: string;
  clientCertPath: string;
  clientKeyPath: string;
}

export const EMPTY_SSL_VALUE: SslFormValue = {
  mode: "",
  rootCertPath: "",
  clientCertPath: "",
  clientKeyPath: "",
};

export interface NewConnectionSslTabProps {
  value: SslFormValue;
  onChange: (next: SslFormValue) => void;
}

export function NewConnectionSslTab({
  value,
  onChange,
}: NewConnectionSslTabProps) {
  function patch<K extends keyof SslFormValue>(
    field: K,
    next: SslFormValue[K],
  ) {
    onChange({ ...value, [field]: next });
  }

  return (
    <Flex
      data-testid="new-connection-ssl-tab"
      direction="column"
      gap={4}
    >
      <Field label="sslmode" hint="Choose how the client negotiates TLS.">
        <ModeSelect
          value={value.mode}
          onChange={(next) => patch("mode", next)}
        />
      </Field>

      <Field
        label="Root cert"
        hint="PEM file for the CA that signed the server (optional for `prefer`)."
      >
        <FilePathInput
          testid="new-connection-ssl-root-cert"
          value={value.rootCertPath}
          onChange={(v) => patch("rootCertPath", v)}
          placeholder="/etc/ssl/certs/server-ca.pem"
          dialogTitle="Select root CA certificate"
          extensions={["pem", "crt", "cer"]}
        />
      </Field>

      <Grid gridTemplateColumns="1fr 1fr" gap={3}>
        <Field label="Client cert">
          <FilePathInput
            testid="new-connection-ssl-client-cert"
            value={value.clientCertPath}
            onChange={(v) => patch("clientCertPath", v)}
            placeholder="/etc/ssl/certs/client.pem"
            dialogTitle="Select client certificate"
            extensions={["pem", "crt", "cer"]}
          />
        </Field>
        <Field label="Client key">
          <FilePathInput
            testid="new-connection-ssl-client-key"
            value={value.clientKeyPath}
            onChange={(v) => patch("clientKeyPath", v)}
            placeholder="/etc/ssl/private/client.key"
            dialogTitle="Select client key"
            extensions={["key", "pem"]}
          />
        </Field>
      </Grid>

      <Box
        data-testid="new-connection-ssl-hint"
        fontSize="11px"
        color="fg.subtle"
        bg="bg.muted"
        borderWidth="1px"
        borderColor="border"
        borderRadius="6px"
        px={3}
        py={2}
      >
        Absolute paths resolve from the app disk at connection time.
        Relative paths resolve from the vault. In `verify-full`, the
        cert host must match the configured host.
      </Box>
    </Flex>
  );
}

const Select = chakra("select");

function ModeSelect({
  value,
  onChange,
}: {
  value: SslMode;
  onChange: (next: SslMode) => void;
}) {
  return (
    <Select
      data-testid="new-connection-ssl-mode"
      value={value}
      onChange={(e) => {
        const next = e.target.value;
        if (isSslMode(next)) onChange(next);
      }}
      h="24px"
      px="8px"
      fontFamily="mono"
      fontSize="12px"
      lineHeight={1}
      borderRadius="4px"
      borderWidth="1px"
      borderColor="border"
      bg="bg"
      color="fg"
    >
      {SSL_MODES.map((mode) => (
        <option key={mode || "default"} value={mode}>
          {SSL_MODE_LABEL[mode]}
        </option>
      ))}
    </Select>
  );
}

/** Path input + native file picker button. Picker filters by the
 * supplied extensions; user can still type a path manually. */
function FilePathInput({
  testid,
  value,
  onChange,
  placeholder,
  dialogTitle,
  extensions,
}: {
  testid: string;
  value: string;
  onChange: (next: string) => void;
  placeholder: string;
  dialogTitle: string;
  extensions: string[];
}) {
  const handleBrowse = async () => {
    try {
      const picked = await openFileDialog({
        multiple: false,
        directory: false,
        title: dialogTitle,
        filters: [
          { name: "Certificate / key", extensions },
          { name: "All files", extensions: ["*"] },
        ],
      });
      if (typeof picked === "string" && picked.length > 0) {
        onChange(picked);
      }
    } catch {
      // User dismissed or dialog plugin unavailable in dev. Silent
      // failure — they can still type the path manually.
    }
  };

  return (
    <HStack gap={2} align="center">
      <Input
        data-testid={testid}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        flex={1}
      />
      <IconButton
        data-testid={`${testid}-browse`}
        aria-label="Browse for file"
        title="Browse…"
        variant="ghost"
        size="sm"
        onClick={handleBrowse}
      >
        <LuFolderOpen />
      </IconButton>
    </HStack>
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
      data-testid={`new-connection-ssl-field-${label.toLowerCase().replace(/\s+/g, "-")}`}
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
