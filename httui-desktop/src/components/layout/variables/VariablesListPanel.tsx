// Canvas §6 Variables — list panel.
//
// Header (serif H1 + subtitle + buttons + search row + env pill),
// table headers `1.4fr 1.4fr 1.4fr 1.4fr 60px`, empty body when no
// rows. Real row rendering + sorting + filtering ship in slice 2.
// Pure presentational; consumer owns env list + search state.

import { Box, Flex, Grid, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";
import { LuPlus } from "react-icons/lu";

import { Btn, Input } from "@/components/atoms";
import { MasterDetailListHeader } from "@/components/layout/shared";

import { VAR_RESOLUTION_HINT } from "./variable-scopes";

export interface VariablesListPanelProps {
  /** Active env id rendered in the right-of-search pill. */
  activeEnvName?: string;
  /** Env names rendered as table column headers. */
  envColumnNames: ReadonlyArray<string>;
  searchValue: string;
  onSearchChange: (next: string) => void;
  onImportDotenv?: () => void;
  onCreateNew?: () => void;
  /** Slot — slice 2 plugs `<VariableListRow>` rows here. */
  rowsSlot?: ReactNode;
  /** Slot — plugs `<NewVariableForm>` above the table headers when the consumer is in create mode. */
  inlineFormSlot?: ReactNode;
}

export function VariablesListPanel({
  activeEnvName,
  envColumnNames,
  searchValue,
  onSearchChange,
  onImportDotenv,
  onCreateNew,
  rowsSlot,
  inlineFormSlot,
}: VariablesListPanelProps) {
  return (
    <Flex
      data-testid="variables-list-panel"
      direction="column"
      flex={1}
      minW={0}
      h="full"
    >
      <Box px={5} pt={4}>
        <MasterDetailListHeader
          title="Variables"
          subtitleSlot={
            <Text
              fontSize="11px"
              color="fg.muted"
              data-testid="variables-resolution-hint"
            >
              {VAR_RESOLUTION_HINT}
            </Text>
          }
          actionsSlot={
            <>
              <Btn
                variant="ghost"
                data-testid="variables-import-dotenv"
                onClick={onImportDotenv}
                disabled={!onImportDotenv}
              >
                Import .env
              </Btn>
              <Btn
                variant="primary"
                data-testid="variables-create-new"
                onClick={onCreateNew}
                disabled={!onCreateNew}
              >
                <LuPlus size={12} /> New
              </Btn>
            </>
          }
        />
      </Box>

      <Flex px={5} py={3} gap={3} align="center">
        <Input
          data-testid="variables-search"
          placeholder="Search key, value, scope…"
          value={searchValue}
          onChange={(e) => onSearchChange(e.target.value)}
          flex={1}
        />
        {activeEnvName && (
          <Box
            data-testid="variables-active-env-pill"
            fontFamily="mono"
            fontSize="11px"
            color="fg.muted"
            bg="bg.muted"
            borderWidth="1px"
            borderColor="border"
            borderRadius="999px"
            px={3}
            py={1}
          >
            env: {activeEnvName}
          </Box>
        )}
      </Flex>

      {inlineFormSlot}

      <TableHeaders envColumnNames={envColumnNames} />

      <Box flex={1} overflowY="auto" data-testid="variables-rows">
        {rowsSlot ?? <EmptyHint />}
      </Box>

      <Text
        data-testid="variables-footer-hint"
        fontSize="11px"
        color="fg.subtle"
        textAlign="center"
        py={2}
      >
        ⌘⇧V new · ⌘. session override · click row for detail
      </Text>
    </Flex>
  );
}

function TableHeaders({
  envColumnNames,
}: {
  envColumnNames: ReadonlyArray<string>;
}) {
  const placeholders = Math.max(0, 3 - envColumnNames.length);
  return (
    <Grid
      data-testid="variables-table-headers"
      gridTemplateColumns="1.4fr 1.4fr 1.4fr 1.4fr 60px"
      px={5}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
      fontFamily="mono"
      fontSize="10px"
      fontWeight="bold"
      letterSpacing="0.06em"
      textTransform="uppercase"
      color="fg.subtle"
    >
      <Text as="span">KEY · SCOPE</Text>
      {envColumnNames.slice(0, 3).map((env) => (
        <Text as="span" key={env} data-testid={`variables-env-header-${env}`}>
          {env}
        </Text>
      ))}
      {Array.from({ length: placeholders }).map((_, i) => (
        <Text as="span" key={`ph-${i}`} color="fg.subtle">
          —
        </Text>
      ))}
      <Text as="span" textAlign="right">
        USES
      </Text>
    </Grid>
  );
}

function EmptyHint() {
  return (
    <Flex
      data-testid="variables-empty-hint"
      align="center"
      justify="center"
      h="full"
      minH="120px"
      color="fg.subtle"
      fontSize="12px"
      px={5}
    >
      No variables found in this view. Create one with{" "}
      <Text as="span" fontFamily="mono" mx={1}>
        + New
      </Text>
      or adjust the scope in the sidebar.
    </Flex>
  );
}
