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
      <Box aria-hidden h="18px" w="1px" bg="border" ml={2} flexShrink={0} />
    </HStack>
  );
}
