// User tab — per-machine prefs. V3 cenário 1.

import { Separator, VStack } from "@chakra-ui/react";

import { ColorModePicker } from "./ColorModePicker";
import { DensityPicker } from "./DensityPicker";
import { FontSizePicker } from "./FontSizePicker";
import { ShortcutProfilePicker } from "./ShortcutProfilePicker";
import { AboutSection } from "./AboutSection";

export function SettingsUserTab() {
  return (
    <VStack align="stretch" gap={6} data-testid="settings-user-tab">
      <ColorModePicker />
      <Separator />
      <ShortcutProfilePicker />
      <Separator />
      <FontSizePicker />
      <Separator />
      <DensityPicker />
      <Separator />
      <AboutSection />
    </VStack>
  );
}
