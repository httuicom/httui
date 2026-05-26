import { useCallback } from "react";
import {
  Box,
  Flex,
  Text,
  VStack,
  HStack,
  Separator,
  Button,
  Badge,
  Input,
} from "@chakra-ui/react";
import { NativeSelectRoot, NativeSelectField } from "@chakra-ui/react";
import { LuRotateCcw, LuSun, LuMoon } from "react-icons/lu";
import { useColorMode } from "@/components/ui/color-mode";
import { useSettingsStore } from "@/stores/settings";
import type { ThemeConfig, ModeColors } from "@/lib/theme/config";
import {
  ACCENT_PALETTES,
  GRAY_PALETTES,
  FONT_BODY_OPTIONS,
  FONT_MONO_OPTIONS,
  DENSITY_SCALES,
  SHADOW_OPTIONS,
} from "@/lib/theme/config";
import { THEME_PRESETS } from "@/lib/theme/presets";

// ─── Color swatch component ────────────────────────────────

function ColorSwatch({
  color,
  selected,
  onClick,
  label,
  size = 28,
}: {
  color: string;
  selected: boolean;
  onClick: () => void;
  label: string;
  size?: number;
}) {
  return (
    <Flex direction="column" align="center" gap={1}>
      <Box
        w={`${size}px`}
        h={`${size}px`}
        borderRadius="full"
        bg={color}
        cursor="pointer"
        onClick={onClick}
        borderWidth="2px"
        borderColor={selected ? "fg" : "transparent"}
        outline={selected ? "2px solid" : "none"}
        outlineColor={selected ? color : "transparent"}
        outlineOffset="2px"
        transition="all 0.15s"
        _hover={{ transform: "scale(1.15)" }}
      />
      <Text
        fontSize="2xs"
        color={selected ? "fg" : "fg.muted"}
        fontWeight={selected ? "medium" : "normal"}
      >
        {label}
      </Text>
    </Flex>
  );
}

// ─── Radius preview ─────────────────────────────────────────

function RadiusOption({
  value,
  selected,
  onClick,
}: {
  value: number;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <Flex direction="column" align="center" gap={1}>
      <Box
        w="36px"
        h="24px"
        borderRadius={`${value}px`}
        borderWidth="2px"
        borderColor={selected ? "fg" : "border"}
        bg={selected ? "brand.500" : "transparent"}
        cursor="pointer"
        onClick={onClick}
        transition="all 0.15s"
        _hover={{ borderColor: "fg" }}
        opacity={selected ? 1 : 0.6}
      />
      <Text fontSize="2xs" color={selected ? "fg" : "fg.muted"}>
        {value}px
      </Text>
    </Flex>
  );
}

// ─── Border width option ────────────────────────────────────

function BorderWidthOption({
  value,
  selected,
  onClick,
}: {
  value: number;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <Flex direction="column" align="center" gap={1}>
      <Box
        w="36px"
        h="24px"
        borderRadius="4px"
        borderWidth={`${value}px`}
        borderColor={selected ? "fg" : "border"}
        borderStyle="solid"
        bg={selected ? "bg.subtle" : "transparent"}
        cursor="pointer"
        onClick={onClick}
        transition="all 0.15s"
        _hover={{ borderColor: "fg" }}
      />
      <Text fontSize="2xs" color={selected ? "fg" : "fg.muted"}>
        {value}px
      </Text>
    </Flex>
  );
}

// ─── Color input with native picker ─────────────────────────

function ColorInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Flex align="center" justify="space-between" gap={3}>
      <Text fontSize="xs" color="fg.muted" flex={1}>
        {label}
      </Text>
      <HStack gap={2}>
        <Box
          as="label"
          w="24px"
          h="24px"
          borderRadius="md"
          bg={value}
          borderWidth="1px"
          borderColor="border"
          cursor="pointer"
          overflow="hidden"
          position="relative"
          flexShrink={0}
        >
          <input
            type="color"
            value={value}
            onChange={(e) => onChange(e.target.value)}
            style={{
              position: "absolute",
              inset: 0,
              opacity: 0,
              cursor: "pointer",
              width: "100%",
              height: "100%",
              border: "none",
              padding: 0,
            }}
          />
        </Box>
        <Input
          size="xs"
          w="85px"
          fontFamily="mono"
          fontSize="xs"
          value={value}
          onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
            const v = e.target.value;
            if (/^#[0-9a-fA-F]{0,6}$/.test(v)) onChange(v);
          }}
        />
      </HStack>
    </Flex>
  );
}

// ─── Section wrapper ────────────────────────────────────────

function SettingGroup({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <Box>
      <Text fontWeight="semibold" fontSize="sm" mb={description ? 1 : 3}>
        {title}
      </Text>
      {description && (
        <Text fontSize="xs" color="fg.muted" mb={3}>
          {description}
        </Text>
      )}
      {children}
    </Box>
  );
}

// ─── Main component ─────────────────────────────────────────

export function ThemeSection() {
  const theme = useSettingsStore((s) => s.theme);
  const updateTheme = useSettingsStore((s) => s.updateTheme);
  const resetTheme = useSettingsStore((s) => s.resetTheme);
  const { colorMode } = useColorMode();
  const currentMode = (colorMode === "dark" ? "dark" : "light") as
    | "light"
    | "dark";

  const currentModeColors: ModeColors | null =
    theme.customColors?.[currentMode] ?? null;

  const initCustomColors = useCallback(() => {
    // Capture current computed colors as starting point
    const cs = getComputedStyle(document.documentElement);
    const get = (v: string) => {
      const raw = cs.getPropertyValue(v).trim();
      // If it looks like a hex or rgb, keep it. Fallback to a safe default.
      return raw && raw !== "" ? raw : "#888888";
    };
    const colors: ModeColors = {
      bg: get("--chakra-colors-bg"),
      bgSubtle: get("--chakra-colors-bg-subtle"),
      fg: get("--chakra-colors-fg"),
      fgMuted: get("--chakra-colors-fg-muted"),
      border: get("--chakra-colors-border"),
    };
    const existing = theme.customColors ?? { light: null, dark: null };
    updateTheme({ customColors: { ...existing, [currentMode]: colors } });
  }, [currentMode, theme.customColors, updateTheme]);

  const clearCustomColors = useCallback(() => {
    if (!theme.customColors) return;
    const updated = { ...theme.customColors, [currentMode]: null };
    // If both are null, remove customColors entirely
    if (!updated.light && !updated.dark) {
      updateTheme({ customColors: null });
    } else {
      updateTheme({ customColors: updated });
    }
  }, [currentMode, theme.customColors, updateTheme]);

  const updateModeColor = useCallback(
    (key: keyof ModeColors, value: string) => {
      if (!currentModeColors) return;
      const updated = { ...currentModeColors, [key]: value };
      const existing = theme.customColors ?? { light: null, dark: null };
      updateTheme({ customColors: { ...existing, [currentMode]: updated } });
    },
    [currentMode, currentModeColors, theme.customColors, updateTheme],
  );

  const handleSelectChange = useCallback(
    (key: string) => (e: React.ChangeEvent<HTMLSelectElement>) => {
      updateTheme({ [key]: e.target.value });
    },
    [updateTheme],
  );

  const handleFontSizeChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      updateTheme({ fontSize: Number(e.target.value) });
    },
    [updateTheme],
  );

  return (
    <Flex direction="column" gap={4}>
      {/* Header with reset */}
      <Flex align="center" justify="space-between">
        <Box>
          <Text fontWeight="semibold" fontSize="sm">
            Theme
          </Text>
          <Text fontSize="xs" color="fg.muted">
            Customize the look and feel of the application
          </Text>
        </Box>
        <Button size="xs" variant="ghost" onClick={resetTheme}>
          <LuRotateCcw size={12} />
          <Text ml={1}>Reset</Text>
        </Button>
      </Flex>

      <Separator />

      {/* Presets */}
      <SettingGroup
        title="Presets"
        description="Quick-apply a predefined theme combination"
      >
        <HStack gap={2} flexWrap="wrap">
          {THEME_PRESETS.map((preset) => {
            const accent = ACCENT_PALETTES[preset.config.accentColor];
            const isActive =
              theme.accentColor === preset.config.accentColor &&
              theme.grayTone === preset.config.grayTone &&
              theme.borderRadius === preset.config.borderRadius;
            return (
              <Flex
                key={preset.id}
                direction="column"
                align="center"
                gap={1}
                px={3}
                py={2}
                borderRadius="md"
                borderWidth="1px"
                borderColor={isActive ? "fg" : "border"}
                bg={isActive ? "bg.subtle" : "transparent"}
                cursor="pointer"
                _hover={{ bg: "bg.subtle" }}
                onClick={() => updateTheme(preset.config)}
                transition="all 0.15s"
                minW="72px"
              >
                <HStack gap={1}>
                  <Box
                    w="10px"
                    h="10px"
                    borderRadius="full"
                    bg={accent?.swatch ?? "#888"}
                  />
                  <Box
                    w="16px"
                    h="10px"
                    borderRadius={`${preset.config.borderRadius}px`}
                    borderWidth="1.5px"
                    borderColor={accent?.swatch ?? "#888"}
                  />
                </HStack>
                <Text
                  fontSize="2xs"
                  fontWeight={isActive ? "semibold" : "normal"}
                >
                  {preset.name}
                </Text>
              </Flex>
            );
          })}
        </HStack>
      </SettingGroup>

      <Separator />

      {/* Accent color */}
      <SettingGroup
        title="Accent color"
        description="Primary color used for buttons, links, and highlights"
      >
        <HStack gap={3} flexWrap="wrap">
          {Object.entries(ACCENT_PALETTES).map(([key, palette]) => (
            <ColorSwatch
              key={key}
              color={palette.swatch}
              selected={theme.accentColor === key}
              onClick={() => updateTheme({ accentColor: key })}
              label={palette.label}
            />
          ))}
        </HStack>
      </SettingGroup>

      <Separator />

      {/* Gray tone */}
      <SettingGroup
        title="Gray tone"
        description="Neutral color palette for backgrounds, text, and borders"
      >
        <HStack gap={3} flexWrap="wrap">
          {Object.entries(GRAY_PALETTES).map(([key, palette]) => (
            <ColorSwatch
              key={key}
              color={palette.swatch}
              selected={theme.grayTone === key}
              onClick={() => updateTheme({ grayTone: key })}
              label={palette.label}
            />
          ))}
        </HStack>
      </SettingGroup>

      <Separator />

      {/* Shape */}
      <SettingGroup title="Shape">
        <VStack gap={3} align="stretch">
          {/* Border radius */}
          <Flex align="center" justify="space-between">
            <Flex direction="column" gap={0}>
              <Text fontSize="sm">Border radius</Text>
              <Text fontSize="xs" color="fg.muted">
                Roundness of UI elements
              </Text>
            </Flex>
            <HStack gap={2}>
              {[0, 2, 4, 6, 8, 12, 16].map((r) => (
                <RadiusOption
                  key={r}
                  value={r}
                  selected={theme.borderRadius === r}
                  onClick={() => updateTheme({ borderRadius: r })}
                />
              ))}
            </HStack>
          </Flex>

          {/* Border width */}
          <Flex align="center" justify="space-between">
            <Flex direction="column" gap={0}>
              <Text fontSize="sm">Border width</Text>
              <Text fontSize="xs" color="fg.muted">
                Thickness of element borders
              </Text>
            </Flex>
            <HStack gap={2}>
              {[0, 1, 2].map((w) => (
                <BorderWidthOption
                  key={w}
                  value={w}
                  selected={theme.borderWidth === w}
                  onClick={() => updateTheme({ borderWidth: w })}
                />
              ))}
            </HStack>
          </Flex>
        </VStack>
      </SettingGroup>

      <Separator />

      {/* Typography */}
      <SettingGroup title="Typography">
        <VStack gap={3} align="stretch">
          {/* Body font */}
          <Flex align="center" justify="space-between" gap={4}>
            <Flex direction="column" gap={0} flex={1}>
              <Text fontSize="sm">Body font</Text>
              <Text fontSize="xs" color="fg.muted">
                Font for text and UI elements
              </Text>
            </Flex>
            <NativeSelectRoot size="sm" w="200px">
              <NativeSelectField
                value={theme.fontBody}
                onChange={handleSelectChange("fontBody")}
              >
                {Object.entries(FONT_BODY_OPTIONS).map(([key, opt]) => (
                  <option key={key} value={key}>
                    {opt.label}
                  </option>
                ))}
              </NativeSelectField>
            </NativeSelectRoot>
          </Flex>

          {/* Mono font */}
          <Flex align="center" justify="space-between" gap={4}>
            <Flex direction="column" gap={0} flex={1}>
              <Text fontSize="sm">Monospace font</Text>
              <Text fontSize="xs" color="fg.muted">
                Font for code editors and block inputs
              </Text>
            </Flex>
            <NativeSelectRoot size="sm" w="200px">
              <NativeSelectField
                value={theme.fontMono}
                onChange={handleSelectChange("fontMono")}
              >
                {Object.entries(FONT_MONO_OPTIONS).map(([key, opt]) => (
                  <option key={key} value={key}>
                    {opt.label}
                  </option>
                ))}
              </NativeSelectField>
            </NativeSelectRoot>
          </Flex>

          {/* Font size */}
          <Flex align="center" justify="space-between" gap={4}>
            <Flex direction="column" gap={0} flex={1}>
              <Text fontSize="sm">Base font size</Text>
              <Text fontSize="xs" color="fg.muted">
                Affects code editors and block content
              </Text>
            </Flex>
            <NativeSelectRoot size="sm" w="200px">
              <NativeSelectField
                value={String(theme.fontSize)}
                onChange={handleFontSizeChange}
              >
                {[12, 13, 14, 15, 16].map((s) => (
                  <option key={s} value={s}>
                    {s}px{s === 14 ? " (default)" : ""}
                  </option>
                ))}
              </NativeSelectField>
            </NativeSelectRoot>
          </Flex>
        </VStack>
      </SettingGroup>

      <Separator />

      {/* Density & Effects */}
      <SettingGroup title="Layout & Effects">
        <VStack gap={3} align="stretch">
          {/* Density */}
          <Flex align="center" justify="space-between" gap={4}>
            <Flex direction="column" gap={0} flex={1}>
              <Text fontSize="sm">UI density</Text>
              <Text fontSize="xs" color="fg.muted">
                {DENSITY_SCALES[theme.density]?.description ?? ""}
              </Text>
            </Flex>
            <HStack gap={1}>
              {Object.entries(DENSITY_SCALES).map(([key, opt]) => (
                <Button
                  key={key}
                  size="xs"
                  variant={theme.density === key ? "subtle" : "ghost"}
                  onClick={() =>
                    updateTheme({ density: key as ThemeConfig["density"] })
                  }
                >
                  {opt.label}
                </Button>
              ))}
            </HStack>
          </Flex>

          {/* Shadow */}
          <Flex align="center" justify="space-between" gap={4}>
            <Flex direction="column" gap={0} flex={1}>
              <Text fontSize="sm">Shadows</Text>
              <Text fontSize="xs" color="fg.muted">
                Drop shadow intensity on elevated elements
              </Text>
            </Flex>
            <HStack gap={1}>
              {Object.entries(SHADOW_OPTIONS).map(([key, opt]) => (
                <Button
                  key={key}
                  size="xs"
                  variant={theme.shadow === key ? "subtle" : "ghost"}
                  onClick={() =>
                    updateTheme({ shadow: key as ThemeConfig["shadow"] })
                  }
                >
                  {opt.label}
                </Button>
              ))}
            </HStack>
          </Flex>
        </VStack>
      </SettingGroup>

      {/* Custom color overrides (mode-aware) */}
      <Separator />
      <SettingGroup
        title="Custom colors"
        description={`Fine-tune background, text, and border colors for ${currentMode} mode. Each mode is configured independently.`}
      >
        <Flex align="center" justify="space-between" mb={3}>
          <HStack gap={2}>
            {currentMode === "dark" ? (
              <LuMoon size={14} />
            ) : (
              <LuSun size={14} />
            )}
            <Text fontSize="sm" fontWeight="medium">
              {currentMode === "dark" ? "Dark" : "Light"} mode
            </Text>
            {theme.customColors?.light && (
              <Badge size="xs" variant="subtle" colorPalette="yellow">
                light customized
              </Badge>
            )}
            {theme.customColors?.dark && (
              <Badge size="xs" variant="subtle" colorPalette="purple">
                dark customized
              </Badge>
            )}
          </HStack>
          <Button
            size="xs"
            variant={currentModeColors ? "outline" : "subtle"}
            onClick={currentModeColors ? clearCustomColors : initCustomColors}
          >
            {currentModeColors ? "Reset to palette" : "Customize"}
          </Button>
        </Flex>

        {currentModeColors && (
          <VStack
            gap={2}
            align="stretch"
            p={3}
            borderRadius="md"
            borderWidth="1px"
            borderColor="border"
          >
            <ColorInput
              label="Background"
              value={currentModeColors.bg}
              onChange={(v) => updateModeColor("bg", v)}
            />
            <ColorInput
              label="Surface"
              value={currentModeColors.bgSubtle}
              onChange={(v) => updateModeColor("bgSubtle", v)}
            />
            <ColorInput
              label="Text"
              value={currentModeColors.fg}
              onChange={(v) => updateModeColor("fg", v)}
            />
            <ColorInput
              label="Muted text"
              value={currentModeColors.fgMuted}
              onChange={(v) => updateModeColor("fgMuted", v)}
            />
            <ColorInput
              label="Borders"
              value={currentModeColors.border}
              onChange={(v) => updateModeColor("border", v)}
            />
          </VStack>
        )}

        {!currentModeColors && (
          <Text fontSize="xs" color="fg.muted">
            Colors are derived from the gray tone palette. Click "Customize" to
            override individually.
          </Text>
        )}
      </SettingGroup>

      {/* Current theme summary */}
      <Separator />
      <Box px={3} py={2} borderRadius="md" bg="bg.subtle">
        <HStack gap={2} flexWrap="wrap" fontSize="2xs" color="fg.muted">
          <Badge size="xs" variant="subtle">
            {ACCENT_PALETTES[theme.accentColor]?.label}
          </Badge>
          <Badge size="xs" variant="subtle">
            {GRAY_PALETTES[theme.grayTone]?.label}
          </Badge>
          <Badge size="xs" variant="subtle">
            {theme.borderRadius}px radius
          </Badge>
          <Badge size="xs" variant="subtle">
            {FONT_BODY_OPTIONS[theme.fontBody]?.label}
          </Badge>
          <Badge size="xs" variant="subtle">
            {FONT_MONO_OPTIONS[theme.fontMono]?.label}
          </Badge>
          <Badge size="xs" variant="subtle">
            {theme.fontSize}px
          </Badge>
          <Badge size="xs" variant="subtle">
            {theme.density}
          </Badge>
          <Badge size="xs" variant="subtle">
            {theme.shadow} shadow
          </Badge>
        </HStack>
      </Box>
    </Flex>
  );
}
