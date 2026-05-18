import { Box, Flex, HStack, Text } from "@chakra-ui/react";
import { LuArrowRight } from "react-icons/lu";
import { useGithubStats } from "../hooks/useGithubStats";
import { Pill } from "../components/atoms";
import { Nav } from "./Nav";

// Hero — Fuji photograph background + serif headline +
// scaled product preview window. The wash is a vertical
// gradient using the page bg token, so it works in both
// themes without separate art.
export function Hero() {
  const stats = useGithubStats();
  return (
    <Box as="section" position="relative" bg="bg">
      <Nav />
      {/* Hero content — clean paper bg, no photo. The Fuji painting only
          appears as scenery around the workbench preview below. */}
      <Flex
        direction="column"
        align="center"
        textAlign="center"
        maxW="1080px"
        mx="auto"
        px={{ base: 5, md: 14 }}
        pt={{ base: 8, md: 16 }}
        pb={{ base: 8, md: 12 }}
      >
        <HStack
          gap={1.5}
          px={3}
          py={1}
          rounded="full"
          fontSize="11px"
          bg="color-mix(in oklch, var(--chakra-colors-bg) 80%, transparent)"
          border="1px solid"
          borderColor="border.subtle"
          color="fg.muted"
          mb={{ base: 5, md: 7 }}
          backdropFilter="blur(8px)"
          whiteSpace="nowrap"
          maxW="100%"
          overflow="hidden"
        >
          <Box w="6px" h="6px" rounded="full" bg="ok" flexShrink={0} />
          <Text display={{ base: "none", sm: "inline" }}>
            {stats.version} · open source —
          </Text>
          <Text display={{ base: "inline", sm: "none" }}>
            {stats.version} —
          </Text>
          <Text fontFamily="mono" color="fg.muted">
            local-first · no telemetry
          </Text>
        </HStack>

        <Text
          as="h1"
          fontFamily="heading"
          fontWeight="600"
          fontSize={{
            base: "34px",
            sm: "40px",
            md: "64px",
            lg: "88px",
            xl: "96px",
          }}
          lineHeight={{ base: "1.06", lg: "1.02" }}
          letterSpacing="tighter"
          color="fg"
          textWrap="balance"
          textShadow="0 1px 2px color-mix(in oklch, var(--chakra-colors-bg) 50%, transparent)"
        >
          Debug your APIs and databases in a{" "}
          <Text as="em" fontStyle="italic">
            single markdown file.
          </Text>
        </Text>

        <Text
          mt={{ base: 5, md: 7 }}
          maxW="620px"
          fontFamily="heading"
          fontSize={{ base: "15px", md: "18px" }}
          lineHeight="1.55"
          color="fg"
        >
          httui is a markdown editor with executable blocks — HTTP requests
          and SQL (PostgreSQL, MySQL, SQLite). Each runbook is documentation
          and a troubleshooting tool, versioned in git.
        </Text>

        <HStack
          gap={3}
          mt={{ base: 7, md: 9 }}
          mb={{ base: 10, md: 14 }}
          flexWrap="wrap"
          justify="center"
        >
          <Pill variant="solid" href={stats.repoUrl}>
            Get started <LuArrowRight size={11} />
          </Pill>
        </HStack>
      </Flex>
    </Box>
  );
}
