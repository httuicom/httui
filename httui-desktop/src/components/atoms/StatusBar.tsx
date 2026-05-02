// Status-bar shell atom — `docs-llm/v1/design-canvas-microdetails.md`
// §0. 22px tall, 11px mono, gap 14, `bg.1` bg, top border `line`.
//
// Pure container. Consumers (workbench `<StatusBarShell>`,
// chat-status, etc.) compose `<Dot>`, `<Kbd>`, text, etc. as
// children. Distinct from `layout/StatusBar.tsx` which is the wired-
// up feature component.

import { HStack, type StackProps } from "@chakra-ui/react";

export type StatusBarShellProps = StackProps;

export function StatusBarShell({ children, ...rest }: StatusBarShellProps) {
  return (
    <HStack
      data-atom="statusbar"
      h="22px"
      px="14px"
      gap="14px"
      bg="bg.subtle"
      borderTopWidth="1px"
      borderTopColor="border"
      fontFamily="mono"
      fontSize="11px"
      color="fg.1"
      flexShrink={0}
      {...rest}
    >
      {children}
    </HStack>
  );
}
