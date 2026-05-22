// "Connection string" tab: user pastes a postgres:// or mysql:// URL; "Fill form"
// dispatches onApply({ kind, value, ssl }) so the consumer can patch state and
// switch back to the Form tab.

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
  initial?: string;
  onApply: (args: NewConnectionStringApplyArgs) => void;
  /** Drives the placeholder example (MySQL vs Postgres). */
  kind?: ConnectionKind;
}

const POSTGRES_EXAMPLE =
  "postgres://orders_app:hunter2@db.internal:5432/orders?sslmode=require";
const MYSQL_EXAMPLE =
  "mysql://orders_app:hunter2@db.internal:3306/orders?ssl-mode=REQUIRED";

function exampleForKind(kind: ConnectionKind | undefined): string {
  return kind === "mysql" ? MYSQL_EXAMPLE : POSTGRES_EXAMPLE;
}

export function NewConnectionStringTab({
  initial = "",
  onApply,
  kind,
}: NewConnectionStringTabProps) {
  const [text, setText] = useState(initial);
  const [result, setResult] = useState<ConnectionStringParseResult | null>(
    null,
  );

  function handleParse() {
    const parsed = parseConnectionString(text);
    setResult(parsed);
    if (parsed.ok) {
      onApply({ kind: parsed.kind, value: parsed.value, ssl: parsed.ssl });
    }
  }

  const isMysql = kind === "mysql";

  return (
    <Flex data-testid="new-connection-string-tab" direction="column" gap={3}>
      <Text fontSize="11px" color="fg.muted">
        {isMysql ? (
          <>
            Paste a <Mono>mysql://</Mono> URL. Form fields and the{" "}
            <Mono>ssl-mode</Mono> param are filled from the URL.
          </>
        ) : (
          <>
            Paste a <Mono>postgres://</Mono> or <Mono>postgresql://</Mono> URL.
            Form fields and the <Mono>sslmode</Mono> /<Mono>sslrootcert</Mono>{" "}
            params are filled from the URL.
          </>
        )}
      </Text>

      <Textarea
        data-testid="new-connection-string-input"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={exampleForKind(kind)}
        rows={6}
      />

      <Flex align="center" gap={2}>
        <Btn
          variant="primary"
          data-testid="new-connection-string-apply"
          onClick={handleParse}
          disabled={text.trim().length === 0}
        >
          Fill form
        </Btn>
        <Box flex={1} />
        {result?.ok && (
          <Text
            data-testid="new-connection-string-success"
            fontSize="11px"
            color="ok"
          >
            Done · fields filled from the URL.
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
