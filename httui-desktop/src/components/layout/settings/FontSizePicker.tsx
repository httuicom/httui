// Editor font-size picker for Settings → User. V3 cenário 1.2.

import { useCallback } from "react";
import {
  Flex,
  NativeSelectField,
  NativeSelectRoot,
  Text,
  VStack,
} from "@chakra-ui/react";

import { useSettingsStore } from "@/stores/settings";

const OPTIONS = [
  { value: "10", label: "10px" },
  { value: "11", label: "11px" },
  { value: "12", label: "12px (default)" },
  { value: "13", label: "13px" },
  { value: "14", label: "14px" },
  { value: "16", label: "16px" },
];

export function FontSizePicker() {
  const editorFontSize = useSettingsStore((s) => s.settings.editorFontSize);
  const updateSetting = useSettingsStore((s) => s.updateSetting);

  const onChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      updateSetting("editorFontSize", Number(e.target.value));
    },
    [updateSetting],
  );

  return (
    <VStack align="stretch" gap={2} data-testid="font-size-picker">
      <Text fontSize="sm" fontWeight={600} color="fg">
        Editor font size
      </Text>
      <Text fontSize="xs" color="fg.muted">
        Applies to code editors and executable blocks.
      </Text>
      <Flex>
        <NativeSelectRoot size="sm" w="180px">
          <NativeSelectField
            aria-label="Editor font size"
            value={String(editorFontSize)}
            onChange={onChange}
          >
            {OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </NativeSelectField>
        </NativeSelectRoot>
      </Flex>
    </VStack>
  );
}
