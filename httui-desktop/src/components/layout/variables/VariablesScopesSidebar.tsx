import { Box, Flex } from "@chakra-ui/react";
import { LuKey } from "react-icons/lu";

import {
  MASTER_DETAIL_SIDEBAR_WIDTH,
  MasterDetailSidebarRow,
  SectionLabel,
  SidebarHintCard,
} from "@/components/layout/shared";

import {
  VARIABLE_HELPERS,
  VARIABLE_SCOPE_META,
  VARIABLE_SCOPES,
  type VariableScope,
} from "./variable-scopes";

export interface VariablesScopesSidebarProps {
  selectedScope: VariableScope;
  onSelectScope: (next: VariableScope) => void;
  /** Per-scope total counts (canvas spec: "All 8 / Workspace 3 / …"). */
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
      w={MASTER_DETAIL_SIDEBAR_WIDTH}
      minW={MASTER_DETAIL_SIDEBAR_WIDTH}
      borderRightWidth="1px"
      borderRightColor="border"
      bg="bg.subtle"
      h="full"
    >
      <SectionLabel px={3} py={2}>
        SCOPES
      </SectionLabel>
      <Flex direction="column" px={2} gap={0.5}>
        {VARIABLE_SCOPES.map((scope) => {
          const meta = VARIABLE_SCOPE_META[scope];
          const active = selectedScope === scope;
          const count = countsByScope?.[scope] ?? 0;
          return (
            <MasterDetailSidebarRow
              key={scope}
              testId={`variables-scope-${scope}`}
              countTestId={`variables-scope-${scope}-count`}
              iconSlot={
                <Box
                  as="span"
                  color="fg.muted"
                  display="inline-flex"
                  alignItems="center"
                  justifyContent="center"
                >
                  <meta.icon size={18} />
                </Box>
              }
              label={meta.label}
              count={count}
              selected={active}
              onClick={() => onSelectScope(scope)}
            />
          );
        })}
      </Flex>

      <SectionLabel px={3} py={2} mt={4}>
        HELPERS
      </SectionLabel>
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

      <Box m={3}>
        <SidebarHintCard
          icon={LuKey}
          title="Local secrets"
          testId="variables-secrets-hint"
        >
          Value lives in the keychain. Other device → re-enter.
        </SidebarHintCard>
      </Box>
    </Flex>
  );
}
