// Color-mode picker for Settings → General. Three radio cells:
// System / Light / Dark. Persists via `useSettingsStore.setColorMode`,
// which writes through to `user.toml [ui].color_mode` and triggers
// `<ColorModeSync />` to re-apply the Chakra mode.

import { chakra, HStack, Text, VStack } from "@chakra-ui/react";
import { LuLaptop, LuMoon, LuSun } from "react-icons/lu";

import { useSettingsStore, type ColorMode } from "@/stores/settings";

const ModeButton = chakra("button");

const OPTIONS: ReadonlyArray<{
  value: ColorMode;
  label: string;
  Icon: React.ComponentType<{ size?: number }>;
}> = [
  { value: "system", label: "System", Icon: LuLaptop },
  { value: "light", label: "Light", Icon: LuSun },
  { value: "dark", label: "Dark", Icon: LuMoon },
];

export function ColorModePicker() {
  const colorMode = useSettingsStore((s) => s.colorMode);
  const setColorMode = useSettingsStore((s) => s.setColorMode);

  return (
    <VStack align="stretch" gap={2} data-testid="color-mode-picker">
      <Text fontSize="sm" fontWeight={600} color="fg">
        Color mode
      </Text>
      <Text fontSize="xs" color="fg.muted">
        Switch between Fuji at dusk (dark) and Fuji photo (light), or
        follow the OS preference.
      </Text>
      <HStack
        role="radiogroup"
        aria-label="Color mode"
        gap={0}
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        overflow="hidden"
        alignSelf="flex-start"
      >
        {OPTIONS.map((opt, idx) => {
          const active = opt.value === colorMode;
          return (
            <ModeButton
              type="button"
              key={opt.value}
              role="radio"
              aria-checked={active}
              data-color-mode={opt.value}
              data-active={active ? "true" : "false"}
              onClick={() => setColorMode(opt.value)}
              h="32px"
              px={3}
              gap={2}
              display="inline-flex"
              alignItems="center"
              fontSize="sm"
              fontWeight={active ? 600 : 500}
              color={active ? "accent.fg" : "fg.muted"}
              bg={active ? "accent" : "transparent"}
              borderLeftWidth={idx === 0 ? 0 : "1px"}
              borderLeftColor="border"
              cursor="pointer"
              _hover={active ? undefined : { bg: "bg.muted", color: "fg" }}
            >
              <opt.Icon size={14} />
              {opt.label}
            </ModeButton>
          );
        })}
      </HStack>
    </VStack>
  );
}
