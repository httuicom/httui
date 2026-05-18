import { Box, Flex, HStack, Text } from "@chakra-ui/react";
import { LuArrowRight } from "react-icons/lu";
import { useGithubStats } from "../hooks/useGithubStats";
import { Logo, Pill } from "../components/atoms";

// Nav — sticky top bar with backdrop blur + repo star count.
// Lives inside the Hero so the photo bleeds behind it.
export function Nav() {
  const stats = useGithubStats();
  return (
    <Box
      position="sticky"
      top={0}
      zIndex={50}
      bg="bg"
      borderBottom="1px solid"
      borderColor="border.subtle"
    >
      <Flex
        align="center"
        maxW="1280px"
        mx="auto"
        width="100%"
        px={{ base: 5, md: 8 }}
        py={3}
        fontSize="sm"
      >
        <Logo variant="full" size={22} />
        <HStack
          flex="1"
          justify="center"
          gap={1}
          display={{ base: "none", md: "flex" }}
        >
          {[
            { t: "Install", href: "#install" },
            {
              t: "Docs",
              href: `${stats.repoUrl}/blob/main/docs/getting-started.md`,
            },
            { t: "Changelog", href: `${stats.repoUrl}/blob/main/CHANGELOG.md` },
            { t: "GitHub", href: stats.repoUrl },
          ].map((l) => (
            <Text
              key={l.t}
              as="a"
              href={l.href}
              {...(l.href.startsWith("#")
                ? {}
                : { target: "_blank", rel: "noreferrer" })}
              px={3}
              py={1.5}
              fontSize="13px"
              fontWeight="500"
              color="fg.muted"
              rounded="md"
              cursor="pointer"
              _hover={{ color: "fg" }}
            >
              {l.t}
            </Text>
          ))}
        </HStack>
        <HStack gap={3}>
          <HStack
            gap={1}
            fontSize="12px"
            fontWeight="500"
            color="fg.muted"
            display={{ base: "none", md: "flex" }}
          >
            <Text as="span">★</Text>
            <Text>{stats.stars}</Text>
          </HStack>
          <Pill variant="ink" size="sm" href={stats.repoUrl}>
            View on GitHub <LuArrowRight size={11} />
          </Pill>
        </HStack>
      </Flex>
    </Box>
  );
}
