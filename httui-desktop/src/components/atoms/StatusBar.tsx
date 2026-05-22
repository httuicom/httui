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
