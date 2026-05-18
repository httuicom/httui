import { Box, Flex, HStack, SimpleGrid, Text, VStack } from "@chakra-ui/react";
import { LuMoon, LuSun } from "react-icons/lu";
import { useColorMode } from "@/components/ui/color-mode";
import { Logo } from "../components/atoms";
import { useGithubStats } from "../hooks/useGithubStats";

export function Footer() {
  const { colorMode, toggleColorMode } = useColorMode();
  const isDark = colorMode === "dark";
  const stats = useGithubStats();
  const repo = stats.repoUrl;
  const blob = `${repo}/blob/main`;
  const cols: { h: string; l: { t: string; href: string }[] }[] = [
    {
      h: "Product",
      l: [
        { t: "Download", href: "#install" },
        { t: "Changelog", href: `${blob}/CHANGELOG.md` },
      ],
    },
    {
      h: "Docs",
      l: [
        { t: "Getting started", href: `${blob}/docs/getting-started.md` },
        { t: "Concepts", href: `${blob}/docs/concepts.md` },
        { t: "Blocks", href: `${blob}/docs/blocks.md` },
        { t: "Chat & MCP", href: `${blob}/docs/chat-mcp.md` },
      ],
    },
    {
      h: "Community",
      l: [
        { t: "GitHub", href: repo },
        { t: "Contributing", href: `${blob}/CONTRIBUTING.md` },
        { t: "Code of Conduct", href: `${blob}/CODE_OF_CONDUCT.md` },
      ],
    },
    {
      h: "Legal",
      l: [
        { t: "MIT License", href: `${blob}/LICENSE` },
        { t: "Security", href: `${blob}/SECURITY.md` },
      ],
    },
  ];
  return (
    <Box
      as="footer"
      px={{ base: 6, md: 20 }}
      pt={14}
      pb={9}
      bg="bg.surface"
      borderTop="1px solid"
      borderColor="border"
      fontSize="xs"
      color="fg.muted"
    >
      <SimpleGrid columns={{ base: 2, md: 5 }} gap={10} maxW="1280px" mx="auto">
        <Box gridColumn={{ base: "span 2", md: "span 1" }}>
          <Logo variant="logo" size={28} />
          <Text mt={3} fontSize="13px" lineHeight="1.55" maxW="280px">
            The markdown editor for debugging APIs and databases. Open source ·
            MIT · {stats.version}.
          </Text>
        </Box>
        {cols.map((col) => (
          <Box key={col.h}>
            <Text
              fontSize="11px"
              fontWeight="700"
              letterSpacing="wide"
              color="fg"
              mb={3}
            >
              {col.h}
            </Text>
            <VStack align="stretch" gap={1.5} fontSize="13px" color="fg.muted">
              {col.l.map((x) => (
                <Text
                  key={x.t}
                  as="a"
                  href={x.href}
                  {...(x.href.startsWith("#")
                    ? {}
                    : { target: "_blank", rel: "noreferrer" })}
                  cursor="pointer"
                  _hover={{ color: "fg" }}
                >
                  {x.t}
                </Text>
              ))}
            </VStack>
          </Box>
        ))}
      </SimpleGrid>
      <Flex
        mt={10}
        pt={4.5}
        borderTop="1px solid"
        borderColor="border"
        justify="space-between"
        maxW="1280px"
        mx="auto"
        direction={{ base: "column", md: "row" }}
        gap={2}
      >
        <Text>© 2026 httui contributors</Text>
        <HStack gap={3}>
          <Text>Made with markdown.</Text>
          <HStack
            as="button"
            type="button"
            onClick={toggleColorMode}
            aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
            gap={1.5}
            px={2}
            py={1}
            rounded="full"
            border="1px solid"
            borderColor="border.subtle"
            color="fg.muted"
            cursor="pointer"
            _hover={{ color: "fg", borderColor: "border" }}
            transition="color 0.15s, border-color 0.15s"
          >
            {isDark ? <LuMoon size={11} /> : <LuSun size={11} />}
            <Text fontSize="11px" fontFamily="mono">
              {isDark ? "dark" : "light"}
            </Text>
          </HStack>
        </HStack>
      </Flex>
    </Box>
  );
}
