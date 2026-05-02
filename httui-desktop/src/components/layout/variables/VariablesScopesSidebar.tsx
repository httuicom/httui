// Canvas §6 Variables — 200px scopes sidebar (Epic 43 Story 01).
//
// Three sections: SCOPES list, HELPERS list, hint card pinned at the
// bottom. Pure presentational; `selectedScope` + `onSelectScope` and
// `countsByScope` drive the rendering. Counts default to 0 when the
// consumer omits a scope.

import { Box, Flex, Text } from "@chakra-ui/react";

import {
  VARIABLE_HELPERS,
  VARIABLE_SCOPE_META,
  VARIABLE_SCOPES,
  type VariableScope,
} from "./variable-scopes";

export interface VariablesScopesSidebarProps {
  selectedScope: VariableScope;
  onSelectScope: (next: VariableScope) => void;
  /** Per-scope total counts (canvas spec: "Todas 8 / Workspace 3 / …"). */
  countsByScope?: Partial<Record<VariableScope, number>>;
}

export function VariablesScopesSidebar({
  selectedScope,
  onSelectScope,
  countsByScope,
}: VariablesScopesSidebarProps) {
  return (
    <Flex
      data-testid="variables-scopes-sidebar"
      direction="column"
      w="200px"
      minW="200px"
      borderRightWidth="1px"
      borderRightColor="border"
      bg="bg.muted"
      h="full"
    >
      <SectionLabel>SCOPES</SectionLabel>
      <Flex direction="column" px={2} gap={1}>
        {VARIABLE_SCOPES.map((scope) => {
          const meta = VARIABLE_SCOPE_META[scope];
          const active = selectedScope === scope;
          const count = countsByScope?.[scope] ?? 0;
          return (
            <Flex
              key={scope}
              data-testid={`variables-scope-${scope}`}
              data-active={active || undefined}
              role="button"
              tabIndex={0}
              align="center"
              gap={2}
              px={2}
              py={1.5}
              borderRadius="6px"
              bg={active ? "bg.emphasized" : "transparent"}
              cursor="pointer"
              fontSize="12px"
              borderLeftWidth={active ? "2px" : "0"}
              borderLeftColor="brand.fg"
              onClick={() => onSelectScope(scope)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onSelectScope(scope);
                }
              }}
              _hover={{ bg: active ? "bg.emphasized" : "bg.subtle" }}
            >
              <Text
                as="span"
                aria-hidden
                w="18px"
                textAlign="center"
                fontSize="12px"
              >
                {meta.glyph}
              </Text>
              <Text as="span" flex={1} truncate color="fg">
                {meta.label}
              </Text>
              <Text
                as="span"
                fontFamily="mono"
                fontSize="11px"
                color="fg.muted"
                data-testid={`variables-scope-${scope}-count`}
              >
                {count}
              </Text>
            </Flex>
          );
        })}
      </Flex>

      <SectionLabel mt={4}>HELPERS</SectionLabel>
      <Flex direction="column" px={2} gap={0.5}>
        {VARIABLE_HELPERS.map((helper) => (
          <Box
            key={helper.syntax}
            data-testid={`variables-helper-${helper.syntax}`}
            fontFamily="mono"
            fontSize="11px"
            color="brand.fg"
            px={2}
            py={1}
            borderRadius="4px"
            title={helper.hint}
            _hover={{ bg: "bg.subtle" }}
          >
            {helper.syntax}
          </Box>
        ))}
      </Flex>

      <Box flex={1} />

      <Box
        data-testid="variables-secrets-hint"
        m={3}
        p={3}
        bg="bg"
        borderWidth="1px"
        borderColor="border"
        borderRadius="6px"
        fontSize="10px"
        color="fg.muted"
        lineHeight={1.4}
      >
        🔑{" "}
        <Text as="span" fontWeight="bold">
          Secrets locais
        </Text>{" "}
        — Valor vive no keychain. Outro device → recadastra.
      </Box>
    </Flex>
  );
}

function SectionLabel({
  children,
  ...rest
}: {
  children: React.ReactNode;
  [k: string]: unknown;
}) {
  return (
    <Text
      as="div"
      fontFamily="mono"
      fontSize="10px"
      fontWeight="bold"
      letterSpacing="0.06em"
      textTransform="uppercase"
      color="fg.subtle"
      px={3}
      py={2}
      {...rest}
    >
      {children}
    </Text>
  );
}
