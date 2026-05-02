// Shortcut profile picker for Settings → User. V3 cenário 1.2.
// Four pills: Default / Vim functional; VSCode / JetBrains surface
// disabled with a "Coming soon" tooltip.

import { chakra, HStack, Text, VStack } from "@chakra-ui/react";

import {
  useSettingsStore,
  type ShortcutProfile,
} from "@/stores/settings";

const ProfilePill = chakra("button");

interface Option {
  value: ShortcutProfile;
  label: string;
  available: boolean;
}

const OPTIONS: ReadonlyArray<Option> = [
  { value: "default", label: "Default", available: true },
  { value: "vim", label: "Vim", available: true },
  { value: "vscode", label: "VS Code", available: false },
  { value: "jetbrains", label: "JetBrains", available: false },
];

export function ShortcutProfilePicker() {
  const profile = useSettingsStore((s) => s.shortcutProfile);
  const setProfile = useSettingsStore((s) => s.setShortcutProfile);

  return (
    <VStack align="stretch" gap={2} data-testid="shortcut-profile-picker">
      <Text fontSize="sm" fontWeight={600} color="fg">
        Keyboard shortcuts profile
      </Text>
      <Text fontSize="xs" color="fg.muted">
        Default and Vim are functional. VS Code and JetBrains profiles
        are coming soon.
      </Text>
      <HStack
        role="radiogroup"
        aria-label="Keyboard shortcuts profile"
        gap={0}
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        overflow="hidden"
        alignSelf="flex-start"
      >
        {OPTIONS.map((opt, idx) => {
          const active = opt.value === profile;
          const disabled = !opt.available;
          return (
            <ProfilePill
              type="button"
              key={opt.value}
              role="radio"
              aria-checked={active}
              aria-disabled={disabled}
              data-shortcut-profile={opt.value}
              data-active={active ? "true" : "false"}
              data-disabled={disabled ? "true" : undefined}
              title={disabled ? "Coming soon" : undefined}
              onClick={() => {
                if (disabled) return;
                setProfile(opt.value);
              }}
              h="32px"
              px={3}
              display="inline-flex"
              alignItems="center"
              fontSize="sm"
              fontWeight={active ? 600 : 500}
              color={
                disabled
                  ? "fg.subtle"
                  : active
                    ? "brand.contrast"
                    : "fg.muted"
              }
              bg={active ? "brand.fg" : "transparent"}
              borderLeftWidth={idx === 0 ? 0 : "1px"}
              borderLeftColor="border"
              cursor={disabled ? "not-allowed" : "pointer"}
              opacity={disabled ? 0.55 : 1}
              _hover={
                disabled || active
                  ? undefined
                  : { bg: "bg.muted", color: "fg" }
              }
            >
              {opt.label}
            </ProfilePill>
          );
        })}
      </HStack>
    </VStack>
  );
}
