// Tabbar shell atom — `docs-llm/v1/design-canvas-microdetails.md` §0.
// 32px tall container; the active tab has a 1px **top** accent line
// (canvas: top, NOT bottom). The atom owns the strip + tab visuals;
// consumers wire id/onSelect/active state.

import { chakra, HStack, type StackProps } from "@chakra-ui/react";
import type { ReactNode } from "react";

const TabButton = chakra("button");

export type TabItem = {
  id: string;
  label: ReactNode;
};

export type TabbarProps = Omit<StackProps, "children" | "onSelect"> & {
  tabs: TabItem[];
  activeId: string | null;
  onSelect: (id: string) => void;
};

export function Tabbar({ tabs, activeId, onSelect, ...rest }: TabbarProps) {
  return (
    <HStack
      data-atom="tabbar"
      role="tablist"
      h="32px"
      px="0"
      gap={0}
      bg="bg.subtle"
      borderBottomWidth="1px"
      borderBottomColor="border"
      flexShrink={0}
      {...rest}
    >
      {tabs.map((t) => {
        const active = t.id === activeId;
        return (
          <TabButton
            type="button"
            key={t.id}
            data-tab-id={t.id}
            data-active={active ? "true" : "false"}
            role="tab"
            aria-selected={active}
            onClick={() => onSelect(t.id)}
            h="32px"
            px="14px"
            display="inline-flex"
            alignItems="center"
            fontFamily="body"
            fontSize="13px"
            fontWeight={active ? 600 : 500}
            color={active ? "fg" : "fg.muted"}
            bg="transparent"
            cursor="pointer"
            borderTopWidth="1px"
            borderTopStyle="solid"
            borderTopColor={active ? "brand.fg" : "transparent"}
            _hover={{ color: "fg" }}
          >
            {t.label}
          </TabButton>
        );
      })}
    </HStack>
  );
}
