// log filter control.
//
// Single input + author/path mode toggle. Pure presentational —
// `LogFilterState` lives in the consumer, which decides whether to
// re-fetch (`path` mode) or filter the in-memory list (`author`
// mode).

import { Box, Flex } from "@chakra-ui/react";
import { LuX } from "react-icons/lu";

import { Btn } from "@/components/atoms";
import { Input } from "@/components/atoms";

import type { LogFilterState } from "./git-log-filter";

export interface GitLogFilterProps {
  state: LogFilterState;
  onChange: (next: LogFilterState) => void;
}

export function GitLogFilter({ state, onChange }: GitLogFilterProps) {
  return (
    <Flex
      data-testid="git-log-filter"
      data-mode={state.mode}
      align="center"
      gap={2}
      px={3}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
      bg="bg.subtle"
    >
      <Box flex={1} minW={0}>
        <Input
          data-testid="git-log-filter-input"
          placeholder={
            state.mode === "author" ? "Filter by author…" : "Filter by path…"
          }
          value={state.query}
          onChange={(e) => onChange({ ...state, query: e.target.value })}
        />
      </Box>
      <Btn
        variant="ghost"
        data-testid="git-log-filter-mode-author"
        data-active={state.mode === "author" || undefined}
        onClick={() => onChange({ ...state, mode: "author" })}
      >
        Author
      </Btn>
      <Btn
        variant="ghost"
        data-testid="git-log-filter-mode-path"
        data-active={state.mode === "path" || undefined}
        onClick={() => onChange({ ...state, mode: "path" })}
      >
        Path
      </Btn>
      {state.query.length > 0 && (
        <Box
          as="button"
          data-testid="git-log-filter-clear"
          aria-label="Clear filter"
          color="fg.subtle"
          flexShrink={0}
          display="inline-flex"
          onClick={() => onChange({ ...state, query: "" })}
        >
          <LuX size={12} />
        </Box>
      )}
    </Flex>
  );
}
