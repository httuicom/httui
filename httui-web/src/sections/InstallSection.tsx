import { Box, HStack, SimpleGrid, Text, VStack } from "@chakra-ui/react";
import { Eyebrow } from "../components/atoms";

function InstallTerminal() {
  return (
    <Box
      maxW="760px"
      mx="auto"
      mb={7}
      rounded="xl"
      overflow="hidden"
      border="1px solid"
      borderColor="stone.500"
      bg="stone.900"
      color="paper.100"
      fontFamily="mono"
      shadow="photo"
    >
      <HStack
        px={3.5}
        py={2.5}
        gap={2}
        borderBottom="1px solid"
        borderColor="stone.500"
      >
        <Box w="10px" h="10px" rounded="full" bg="#ed6a5e" />
        <Box w="10px" h="10px" rounded="full" bg="#f4be4f" />
        <Box w="10px" h="10px" rounded="full" bg="#62c554" />
        <Text flex="1" textAlign="center" fontSize="11px" color="stone.200">
          ~/projects · zsh
        </Text>
      </HStack>
      <Box px={5} py={5} fontSize="14px" lineHeight="1.8">
        <Text>
          <Text as="span" color="moss.300">
            $
          </Text>{" "}
          curl -fsSL https://httui.com/install.sh | sh
        </Text>
        <Text color="stone.200" fontSize="13px">
          ✓ downloading the latest release…
        </Text>
        <Text color="stone.200" fontSize="13px">
          ✓ installed httui → /Applications/httui.app
        </Text>
        <Text>
          <Text as="span" color="moss.300">
            $
          </Text>{" "}
          open /Applications/httui.app
        </Text>
      </Box>
    </Box>
  );
}

// InstallSection — primary curl one-liner + the real alternatives.
// Everything here is a command that actually works today.
export function InstallSection() {
  const distros = [
    {
      label: "Homebrew",
      icon: "",
      lines: ["brew tap httuicom/httui", "brew install --cask httui"],
    },
    {
      label: "GitHub Releases",
      icon: "▣",
      lines: [".dmg · .msi · .exe", ".deb · .rpm · .AppImage"],
    },
    {
      label: "From source",
      icon: "{ }",
      lines: ["make install-deps", "cargo tauri build"],
    },
  ];
  return (
    <Box
      as="section"
      id="install"
      px={{ base: 6, md: 20 }}
      py={{ base: 16, md: 24 }}
      bg="bg.surface"
      borderTop="1px solid"
      borderBottom="1px solid"
      borderColor="border"
    >
      <VStack gap={3.5} mb={10} textAlign="center">
        <Eyebrow>Install</Eyebrow>
        <Text
          as="h2"
          fontFamily="heading"
          fontWeight="600"
          fontSize={{ base: "36px", md: "52px" }}
          lineHeight="1.1"
          letterSpacing="tight"
          color="fg"
        >
          Free forever.{" "}
          <Text as="em" color="accent" fontStyle="italic">
            Yours
          </Text>{" "}
          to fork.
        </Text>
        <Text
          fontFamily="heading"
          fontSize="17px"
          color="fg.muted"
          maxW="580px"
        >
          One line. No signup, no card, no telemetry.
        </Text>
      </VStack>

      <InstallTerminal />

      {/* Real alternatives */}
      <SimpleGrid maxW="920px" mx="auto" columns={{ base: 1, md: 3 }} gap={2.5}>
        {distros.map((p) => (
          <Box
            key={p.label}
            p={3.5}
            bg="bg"
            border="1px solid"
            borderColor="border"
            rounded="md"
          >
            <HStack gap={2} mb={2} fontSize="11px" color="fg.subtle">
              <Text fontFamily="mono">{p.icon}</Text>
              <Text fontWeight="600" color="fg.muted">
                {p.label}
              </Text>
            </HStack>
            <VStack align="stretch" gap={0.5}>
              {p.lines.map((line) => (
                <Text
                  key={line}
                  fontFamily="mono"
                  fontSize="11.5px"
                  color="fg"
                  truncate
                >
                  {line}
                </Text>
              ))}
            </VStack>
          </Box>
        ))}
      </SimpleGrid>

      <Text
        textAlign="center"
        mt={7}
        fontSize="xs"
        color="fg.muted"
        maxW="720px"
        mx="auto"
        lineHeight="1.7"
      >
        GUI builds for{" "}
        <Text as="span" fontFamily="mono" color="fg.muted">
          macOS
        </Text>{" "}
        ·{" "}
        <Text as="span" fontFamily="mono" color="fg.muted">
          Linux
        </Text>{" "}
        ·{" "}
        <Text as="span" fontFamily="mono" color="fg.muted">
          Windows
        </Text>{" "}
        on{" "}
        <Text as="span" color="accent.emphasized" fontWeight="600">
          GitHub releases
        </Text>
        . The macOS build is an unsigned developer build — the install
        script and the Homebrew cask clear the Gatekeeper quarantine for
        you, and in-app auto-update keeps it current.
      </Text>
    </Box>
  );
}
