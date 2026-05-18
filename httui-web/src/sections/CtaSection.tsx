import { Box, HStack, Text } from "@chakra-ui/react";
import { useGithubStats } from "../hooks/useGithubStats";
import { Pill } from "../components/atoms";

export function CtaSection() {
  const stats = useGithubStats();
  return (
    <Box
      as="section"
      px={{ base: 6, md: 20 }}
      py={{ base: 20, md: 28 }}
      textAlign="center"
      bgGradient="linear(to-b, var(--chakra-colors-bg) 0%, var(--chakra-colors-bg-surface) 100%)"
      borderTop="1px solid"
      borderColor="border"
    >
      <Text
        as="h2"
        fontFamily="heading"
        fontWeight="600"
        fontSize={{ base: "40px", md: "64px" }}
        lineHeight="1.05"
        letterSpacing="tight"
        color="fg"
        maxW="920px"
        mx="auto"
        textWrap="balance"
      >
        Stop debugging in{" "}
        <Text as="em" fontStyle="italic" color="fg.muted">
          five tabs.
        </Text>
        <br />
        Start writing{" "}
        <Text as="em" color="accent" fontStyle="italic">
          runbooks
        </Text>{" "}
        instead.
      </Text>
      <Text
        mt={5}
        fontFamily="heading"
        fontSize="17px"
        color="fg.muted"
        maxW="540px"
        mx="auto"
      >
        Open source, MIT licensed.{" "}
        <Text
          as="span"
          fontFamily="mono"
          bg="bg.elevated"
          px={1.5}
          py={0.5}
          rounded="sm"
          fontSize="13px"
        >
          curl -fsSL https://httui.com/install.sh | sh
        </Text>{" "}
        and it's yours.
      </Text>
      <HStack gap={3} justify="center" mt={9}>
        <Pill variant="solid" href={stats.repoUrl}>
          Get started
        </Pill>
        <Pill variant="ghost">Read the docs</Pill>
      </HStack>
    </Box>
  );
}
