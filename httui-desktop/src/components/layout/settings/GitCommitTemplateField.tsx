// V10.1 cenário 8 — the configurable commit-message template field.
//
// Extracted from GeneralSection so that section stays within the
// max-lines-per-function budget (SRP — this owns its own store
// wiring). Empty value = the built-in conditional default.

import { Box, Flex, Input, Text } from "@chakra-ui/react";

import { useSettingsStore } from "@/stores/settings";

export function GitCommitTemplateField() {
  const gitCommitTemplate = useSettingsStore((s) => s.gitCommitTemplate);
  const setGitCommitTemplate = useSettingsStore((s) => s.setGitCommitTemplate);

  return (
    <Box>
      <Text fontWeight="semibold" fontSize="sm" mb={3}>
        Git — commit message template
      </Text>
      <Flex direction="column" gap={2}>
        <Text fontSize="xs" color="fg.muted">
          Pre-fills the Source Control commit box. Placeholders:{" "}
          <Text as="span" fontFamily="mono">
            {"{{notes}}"}
          </Text>{" "}
          <Text as="span" fontFamily="mono">
            {"{{count}}"}
          </Text>{" "}
          <Text as="span" fontFamily="mono">
            {"{{date}}"}
          </Text>
          . Leave empty for the smart default (&quot;Update &lt;note&gt;&quot; /
          &quot;Update N notes&quot;).
        </Text>
        <Input
          data-testid="git-commit-template-input"
          size="sm"
          fontFamily="mono"
          placeholder="Update {{notes}}"
          value={gitCommitTemplate}
          onChange={(e) => setGitCommitTemplate(e.target.value)}
        />
      </Flex>
    </Box>
  );
}
