import { Box, Flex, Text, VStack, Badge, Separator } from "@chakra-ui/react";

const APP_VERSION = "0.1.0";

interface InfoRow {
  label: string;
  value: string;
}

const TECH_STACK: InfoRow[] = [
  { label: "Frontend", value: "React + TypeScript + Chakra UI v3" },
  { label: "Editor", value: "CodeMirror 6" },
  { label: "Backend", value: "Tauri v2 (Rust)" },
  { label: "Database", value: "SQLite (sqlx)" },
  { label: "AI", value: "Claude (Anthropic SDK)" },
];

export function AboutSection() {
  return (
    <Flex direction="column" gap={4}>
      {/* App identity */}
      <Flex align="center" gap={3}>
        <Box>
          <Flex align="center" gap={2}>
            <Text fontWeight="semibold" fontSize="sm">
              Notes
            </Text>
            <Badge size="sm" variant="subtle" colorPalette="purple">
              v{APP_VERSION}
            </Badge>
          </Flex>
          <Text fontSize="xs" color="fg.muted" mt={0.5}>
            Desktop markdown editor with executable blocks
          </Text>
        </Box>
      </Flex>

      <Separator />

      {/* What it does */}
      <Box>
        <Text fontWeight="medium" fontSize="xs" mb={1.5}>
          About
        </Text>
        <Text fontSize="xs" color="fg.muted" lineHeight="tall">
          Notes is a desktop markdown editor that lets you embed executable
          blocks (HTTP requests, database queries, E2E tests) directly in your
          documents. Results are cached, environments are switchable, and
          credentials are stored in your OS keychain.
        </Text>
      </Box>

      <Separator />

      {/* Tech stack */}
      <Box>
        <Text fontWeight="medium" fontSize="xs" mb={1.5}>
          Built with
        </Text>
        <VStack gap={1} align="stretch">
          {TECH_STACK.map((item) => (
            <Flex key={item.label} justify="space-between" fontSize="xs">
              <Text color="fg.muted">{item.label}</Text>
              <Text fontWeight="medium">{item.value}</Text>
            </Flex>
          ))}
        </VStack>
      </Box>

      <Separator />

      {/* Security summary */}
      <Box>
        <Text fontWeight="medium" fontSize="xs" mb={1.5}>
          Security
        </Text>
        <VStack gap={1.5} align="stretch" fontSize="xs" color="fg.muted">
          <Flex align="center" gap={2}>
            <Badge size="xs" colorPalette="green" variant="subtle">
              Keychain
            </Badge>
            <Text>Passwords and secrets stored in OS keychain</Text>
          </Flex>
          <Flex align="center" gap={2}>
            <Badge size="xs" colorPalette="green" variant="subtle">
              Read-only
            </Badge>
            <Text>Internal database queries are SELECT-only</Text>
          </Flex>
          <Flex align="center" gap={2}>
            <Badge size="xs" colorPalette="green" variant="subtle">
              Sandboxed
            </Badge>
            <Text>SQL injection prevented via parameterized queries</Text>
          </Flex>
          <Flex align="center" gap={2}>
            <Badge size="xs" colorPalette="green" variant="subtle">
              Signed
            </Badge>
            <Text>Sidecar protocol HMAC-signed</Text>
          </Flex>
        </VStack>
      </Box>

      <Separator />

      {/* Data */}
      <Box>
        <Text fontWeight="medium" fontSize="xs" mb={1.5}>
          Data storage
        </Text>
        <VStack gap={1} align="stretch" fontSize="xs" color="fg.muted">
          <Text>
            App data (connections, cache, chat history) stored in{" "}
            <Text as="span" fontFamily="mono" fontWeight="medium" color="fg">
              notes.db
            </Text>{" "}
            in the OS app data directory. File permissions set to owner-only
            (0600).
          </Text>
          <Text>
            Notes stored as{" "}
            <Text as="span" fontFamily="mono" fontWeight="medium" color="fg">
              .md
            </Text>{" "}
            files in your vault directory. Executable blocks serialized as YAML
            in fenced code blocks.
          </Text>
        </VStack>
      </Box>
    </Flex>
  );
}
