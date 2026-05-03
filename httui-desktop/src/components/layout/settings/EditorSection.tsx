import { useCallback } from "react";
import { Flex, Text, Separator, Box, VStack } from "@chakra-ui/react";
import { NativeSelectRoot, NativeSelectField } from "@chakra-ui/react";
import { useSettingsStore } from "@/stores/settings";
import { Switch } from "@/components/ui/switch";

const FONT_SIZE_OPTIONS = [
  { value: "10", label: "10px" },
  { value: "11", label: "11px" },
  { value: "12", label: "12px (default)" },
  { value: "13", label: "13px" },
  { value: "14", label: "14px" },
  { value: "16", label: "16px" },
];

const FETCH_SIZE_OPTIONS = [
  { value: "20", label: "20 rows" },
  { value: "50", label: "50 rows" },
  { value: "80", label: "80 rows (default)" },
  { value: "100", label: "100 rows" },
  { value: "200", label: "200 rows" },
  { value: "500", label: "500 rows" },
];

export function EditorSection() {
  const vimEnabled = useSettingsStore((s) => s.vimEnabled);
  const toggleVim = useSettingsStore((s) => s.toggleVim);
  const settings = useSettingsStore((s) => s.settings);
  const updateSetting = useSettingsStore((s) => s.updateSetting);

  const handleFontSizeChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      updateSetting("editorFontSize", Number(e.target.value));
    },
    [updateSetting],
  );

  const handleFetchSizeChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      updateSetting("defaultFetchSize", Number(e.target.value));
    },
    [updateSetting],
  );

  return (
    <Flex direction="column" gap={4}>
      {/* Vim mode */}
      <Box>
        <Text fontWeight="semibold" fontSize="sm" mb={3}>
          Keybindings
        </Text>
        <Flex align="center" justify="space-between">
          <Flex direction="column" gap={0}>
            <Text fontSize="sm">Vim mode</Text>
            <Text fontSize="xs" color="fg.muted">
              Enable Vim keybindings in the text editor
            </Text>
          </Flex>
          <Switch checked={vimEnabled} onCheckedChange={toggleVim} size="sm" />
        </Flex>
        {vimEnabled && (
          <Box
            mt={2}
            px={3}
            py={2}
            borderRadius="md"
            bg="bg.subtle"
            fontSize="xs"
            color="fg.muted"
          >
            <Text lineHeight="tall">
              Vim mode adds Normal, Insert, and Visual modes to the editor.
              Motions (j/k/h/l, w/b, gg/G) navigate by ProseMirror textblocks.
              Press{" "}
              <Text as="span" fontFamily="mono" fontWeight="medium" color="fg">
                i
              </Text>{" "}
              to enter Insert mode,{" "}
              <Text as="span" fontFamily="mono" fontWeight="medium" color="fg">
                Esc
              </Text>{" "}
              to return to Normal mode. The current mode is shown in the status
              bar.
            </Text>
          </Box>
        )}
      </Box>

      <Separator />

      {/* Font size */}
      <Box>
        <Text fontWeight="semibold" fontSize="sm" mb={3}>
          Appearance
        </Text>
        <Flex align="center" justify="space-between" gap={4}>
          <Flex direction="column" gap={0} flex={1}>
            <Text fontSize="sm">Editor font size</Text>
            <Text fontSize="xs" color="fg.muted">
              Font size for code editors and executable blocks
            </Text>
          </Flex>
          <NativeSelectRoot size="sm" w="180px">
            <NativeSelectField
              value={String(settings.editorFontSize)}
              onChange={handleFontSizeChange}
            >
              {FONT_SIZE_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </NativeSelectField>
          </NativeSelectRoot>
        </Flex>
      </Box>

      <Separator />

      {/* Block execution */}
      <Box>
        <Text fontWeight="semibold" fontSize="sm" mb={3}>
          Block execution
        </Text>

        <Flex align="center" justify="space-between" gap={4} mb={3}>
          <Flex direction="column" gap={0} flex={1}>
            <Text fontSize="sm">Default fetch size</Text>
            <Text fontSize="xs" color="fg.muted">
              Number of rows loaded per page in DB query results
            </Text>
          </Flex>
          <NativeSelectRoot size="sm" w="180px">
            <NativeSelectField
              value={String(settings.defaultFetchSize)}
              onChange={handleFetchSizeChange}
            >
              {FETCH_SIZE_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </NativeSelectField>
          </NativeSelectRoot>
        </Flex>

        <VStack gap={1} align="stretch" fontSize="xs" color="fg.muted">
          <Flex justify="space-between">
            <Text>Max fetch size (hard limit)</Text>
            <Text fontWeight="medium" color="fg">
              1,000 rows
            </Text>
          </Flex>
          <Flex justify="space-between">
            <Text>Default query timeout</Text>
            <Text fontWeight="medium" color="fg">
              30s (from connection config)
            </Text>
          </Flex>
          <Flex justify="space-between">
            <Text>HTTP timeout</Text>
            <Text fontWeight="medium" color="fg">
              30s (per-request override)
            </Text>
          </Flex>
          <Flex justify="space-between">
            <Text>Max dependency depth</Text>
            <Text fontWeight="medium" color="fg">
              50 levels
            </Text>
          </Flex>
        </VStack>
      </Box>

      <Separator />

      {/* References */}
      <Box>
        <Text fontWeight="semibold" fontSize="sm" mb={2}>
          Block references
        </Text>
        <Text fontSize="xs" color="fg.muted" lineHeight="tall">
          Use{" "}
          <Text as="span" fontFamily="mono" fontWeight="medium" color="fg">
            {"{{alias.response.path}}"}
          </Text>{" "}
          to reference another block's output. References are resolved top-down
          — a block can only reference blocks above it in the document. In DB
          blocks, references are converted to bind parameters (never
          string-interpolated) for SQL safety.
        </Text>
        <Text fontSize="xs" color="fg.muted" lineHeight="tall" mt={1}>
          Environment variables use the same syntax without dots:{" "}
          <Text as="span" fontFamily="mono" fontWeight="medium" color="fg">
            {"{{ENV_KEY}}"}
          </Text>
          . If a block alias collides with an env variable name, the block
          reference takes priority.
        </Text>
      </Box>
    </Flex>
  );
}
