// Canvas §5 — "Connection string" tab for the Nova Conexão modal
// (Epic 42 Story 06 — Phase 3).
//
// User pastes a `postgres://…` / `mysql://…` URL; clicking "Preencher
// formulário" dispatches `onApply({ kind, value, ssl })` so the
// consumer can patch the form + ssl state and switch back to the
// Form tab. Pure presentational with local state for the textarea
// + last-error surface.

import { useState } from "react";
import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

import {
  parseConnectionString,
  type ConnectionStringParseResult,
} from "./connection-string-parser";
import type { ConnectionKind } from "./connection-kinds";
import type { PostgresFormValue } from "./NewConnectionFormTab";
import type { SslFormValue } from "./NewConnectionSslTab";

export interface NewConnectionStringApplyArgs {
  kind: ConnectionKind;
  value: PostgresFormValue;
  ssl: SslFormValue;
}

export interface NewConnectionStringTabProps {
  /** Initial textarea value (e.g. when reopening with the last paste). */
  initial?: string;
  /** Dispatched when the parsed result is applied to the form. */
  onApply: (args: NewConnectionStringApplyArgs) => void;
}

const PLACEHOLDER =
  "postgres://orders_app:hunter2@db.internal:5432/orders?sslmode=require";

export function NewConnectionStringTab({
  initial = "",
  onApply,
}: NewConnectionStringTabProps) {
  const [text, setText] = useState(initial);
  const [result, setResult] = useState<
    ConnectionStringParseResult | null
  >(null);

  function handleParse() {
    const parsed = parseConnectionString(text);
    setResult(parsed);
    if (parsed.ok) {
      onApply({ kind: parsed.kind, value: parsed.value, ssl: parsed.ssl });
    }
  }

  return (
    <Flex
      data-testid="new-connection-string-tab"
      direction="column"
      gap={3}
    >
      <Text fontSize="11px" color="fg.muted">
        Cole uma URL do tipo <Mono>postgres://</Mono>, <Mono>postgresql://</Mono>{" "}
        ou <Mono>mysql://</Mono>. Os campos do formulário e os parâmetros{" "}
        <Mono>sslmode</Mono> /<Mono>sslrootcert</Mono> são preenchidos a
        partir da URL.
      </Text>

      <Textarea
        data-testid="new-connection-string-input"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={PLACEHOLDER}
        rows={6}
      />

      <Flex align="center" gap={2}>
        <Btn
          variant="primary"
          data-testid="new-connection-string-apply"
          onClick={handleParse}
          disabled={text.trim().length === 0}
        >
          Preencher formulário
        </Btn>
        <Box flex={1} />
        {result?.ok && (
          <Text
            data-testid="new-connection-string-success"
            fontSize="11px"
            color="ok"
          >
            Pronto · campos atualizados a partir da URL.
          </Text>
        )}
      </Flex>

      {result && !result.ok && (
        <Box
          data-testid="new-connection-string-error"
          fontSize="11px"
          color="err"
          bg="bg.muted"
          borderWidth="1px"
          borderColor="err"
          borderRadius="6px"
          px={3}
          py={2}
        >
          {result.error}
        </Box>
      )}
    </Flex>
  );
}

const Textarea = chakra("textarea");

const Mono = ({ children }: { children: React.ReactNode }) => (
  <Text as="span" fontFamily="mono">
    {children}
  </Text>
);
