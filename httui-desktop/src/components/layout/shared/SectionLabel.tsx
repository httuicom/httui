// Master-detail section label (V5).
//
// Uppercase mono caption used by the master-detail sidebars
// (Connections, Variables, Environments) and the detail panel
// composers. Centralises type tokens so layout reads identically
// across surfaces.

import { Text } from "@chakra-ui/react";
import type { ReactNode } from "react";

export interface SectionLabelProps {
  children: ReactNode;
  /** Forwarded Chakra spacing / props (mt, mb, px, etc.). */
  [key: string]: unknown;
}

export function SectionLabel({ children, ...rest }: SectionLabelProps) {
  return (
    <Text
      as="div"
      fontFamily="mono"
      fontSize="11px"
      fontWeight="bold"
      letterSpacing="0.08em"
      textTransform="uppercase"
      color="fg.muted"
      {...rest}
    >
      {children}
    </Text>
  );
}
