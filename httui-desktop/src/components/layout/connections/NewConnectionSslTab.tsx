// Canvas §5 — "SSL" tab for the Nova Conexão modal
// (Epic 42 Story 06 — Phase 3).
//
// Renders the SSL configuration fields: sslmode select + root cert
// path + client cert + client key paths. Pure presentational; the
// consumer owns the value and onChange. Used both as a tab and as
// the secondary patch surface when a connection string carries
// ?sslmode=...&sslrootcert=... params.

import { Box, Flex, Grid, Text, chakra } from "@chakra-ui/react";
import type { ReactNode } from "react";

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
      <Field label="sslmode" hint="Escolha como o cliente negocia TLS.">
        <ModeSelect
          value={value.mode}
          onChange={(next) => patch("mode", next)}
        />
      </Field>

      <Field
        label="Root cert"
        hint="Arquivo PEM da CA que assinou o servidor (opcional para `prefer`)."
      >
        <Input
          data-testid="new-connection-ssl-root-cert"
          value={value.rootCertPath}
          onChange={(e) => patch("rootCertPath", e.target.value)}
          placeholder="/etc/ssl/certs/server-ca.pem"
        />
      </Field>

      <Grid gridTemplateColumns="1fr 1fr" gap={3}>
        <Field label="Client cert">
          <Input
            data-testid="new-connection-ssl-client-cert"
            value={value.clientCertPath}
            onChange={(e) => patch("clientCertPath", e.target.value)}
            placeholder="/etc/ssl/certs/client.pem"
          />
        </Field>
        <Field label="Client key">
          <Input
            data-testid="new-connection-ssl-client-key"
            value={value.clientKeyPath}
            onChange={(e) => patch("clientKeyPath", e.target.value)}
            placeholder="/etc/ssl/private/client.key"
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
        Caminhos absolutos resolvem do disco do app no momento da conexão.
        Caminhos relativos resolvem a partir do vault. Em `verify-full`, o
        host do certificado precisa bater com o host configurado.
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
