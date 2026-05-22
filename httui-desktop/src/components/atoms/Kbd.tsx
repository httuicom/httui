import { Box, type BoxProps } from "@chakra-ui/react";
import type { ReactNode } from "react";

export type KbdProps = Omit<BoxProps, "children"> & {
  children: ReactNode;
};

export function Kbd({ children, ...rest }: KbdProps) {
  return (
    <Box
      as="kbd"
      data-atom="kbd"
      display="inline-flex"
      alignItems="center"
      justifyContent="center"
      minWidth="18px"
      height="18px"
      px="5px"
      fontFamily="mono"
      fontSize="10px"
      fontWeight={500}
      lineHeight={1}
      bg="bg.muted"
      color="fg"
      border="1px solid"
      borderColor="border"
      borderBottomWidth="2px"
      borderRadius="4px"
      userSelect="none"
      {...rest}
    >
      {children}
    </Box>
  );
}
