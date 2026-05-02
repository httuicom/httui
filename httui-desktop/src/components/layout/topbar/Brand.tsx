// httui wordmark + 1×18 vertical divider — canvas §4.
//
// Uses the same logo assets as the marketing landing
// (`httui-web/public/httui-{light,dark}-full.png`, 66×19). Theme-aware
// via `useColorMode` so the dark variant kicks in when the workbench
// switches modes. The PNG is rendered with a fixed height and `width
// auto` so the aspect ratio stays clean.

import { Box, HStack } from "@chakra-ui/react";

import { useColorMode } from "@/components/ui/color-mode";

export function Brand() {
  const { colorMode } = useColorMode();
  const src =
    colorMode === "dark" ? "/httui-dark-full.png" : "/httui-light-full.png";

  return (
    <HStack data-atom="brand" gap={2} flexShrink={0}>
      <img
        src={src}
        alt="httui"
        height={16}
        style={{
          display: "block",
          height: "16px",
          width: "auto",
          maxHeight: "16px",
          flexShrink: 0,
        }}
      />
      <Box
        aria-hidden
        h="18px"
        w="1px"
        bg="line"
        ml={2}
        flexShrink={0}
      />
    </HStack>
  );
}
