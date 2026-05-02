// Density picker for Settings → User. V3 cenário 1.2. Three cells:
// Compact / Comfortable (default) / Spacious. Persists via
// `useSettingsStore.setDensity` which writes through to user.toml and
// updates the `--httui-density` CSS variable.

import { chakra, HStack, Text, VStack } from "@chakra-ui/react";

import { useSettingsStore, type Density } from "@/stores/settings";

const DensityButton = chakra("button");

const OPTIONS: ReadonlyArray<{ value: Density; label: string }> = [
  { value: "compact", label: "Compact" },
  { value: "comfortable", label: "Comfortable" },
  { value: "spacious", label: "Spacious" },
];

export function DensityPicker() {
  const density = useSettingsStore((s) => s.density);
  const setDensity = useSettingsStore((s) => s.setDensity);

  return (
    <VStack align="stretch" gap={2} data-testid="density-picker">
      <Text fontSize="sm" fontWeight={600} color="fg">
        Density
      </Text>
      <Text fontSize="xs" color="fg.muted">
        Scales spacing across the workspace. Use Compact for dense
        screens, Spacious for touchpads.
      </Text>
      <HStack
        role="radiogroup"
        aria-label="UI density"
        gap={0}
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        overflow="hidden"
        alignSelf="flex-start"
      >
        {OPTIONS.map((opt, idx) => {
          const active = opt.value === density;
          return (
            <DensityButton
              type="button"
              key={opt.value}
              role="radio"
              aria-checked={active}
              data-density={opt.value}
              data-active={active ? "true" : "false"}
              onClick={() => setDensity(opt.value)}
              h="32px"
              px={3}
              display="inline-flex"
              alignItems="center"
              fontSize="sm"
              fontWeight={active ? 600 : 500}
              color={active ? "brand.contrast" : "fg.muted"}
              bg={active ? "brand.fg" : "transparent"}
              borderLeftWidth={idx === 0 ? 0 : "1px"}
              borderLeftColor="border"
              cursor="pointer"
              _hover={active ? undefined : { bg: "bg.muted", color: "fg" }}
            >
              {opt.label}
            </DensityButton>
          );
        })}
      </HStack>
    </VStack>
  );
}
