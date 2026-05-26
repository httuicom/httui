import {
  Box,
  Button,
  Field,
  Flex,
  IconButton,
  Input,
  NativeSelectField,
  NativeSelectRoot,
  Portal,
  Text,
} from "@chakra-ui/react";
import { LuTrash2, LuX } from "react-icons/lu";
import type {
  HttpBlockMetadata,
  HttpDisplayMode,
} from "@/lib/blocks/http-fence";
import type {
  BlockExample,
  HistoryEntry,
  HttpBlockSettings,
} from "@/lib/tauri/commands";
import { Switch } from "@/components/ui/switch";
import { relativeTimeAgo, statusDotColor } from "./shared";

interface HttpSettingsDrawerProps {
  metadata: HttpBlockMetadata;
  history: HistoryEntry[];
  examples: BlockExample[];
  settings: HttpBlockSettings;
  canSaveExample: boolean;
  onClose: () => void;
  onUpdateMetadata: (patch: Partial<HttpBlockMetadata>) => void;
  onUpdateSettings: (patch: Partial<HttpBlockSettings>) => void;
  onDelete: () => void;
  onPurgeHistory: () => void;
  onSaveExample: (name: string) => void;
  onRestoreExample: (ex: BlockExample) => void;
  onDeleteExample: (id: number) => void;
}

const PER_BLOCK_FLAGS: Array<{
  key: keyof HttpBlockSettings;
  label: string;
  hint: string;
  defaultOn: boolean;
}> = [
  {
    key: "followRedirects",
    label: "Follow redirects",
    hint: "Disable to inspect 3xx responses directly.",
    defaultOn: true,
  },
  {
    key: "verifySsl",
    label: "Verify SSL",
    hint: "Disable to accept self-signed certificates.",
    defaultOn: true,
  },
  {
    key: "encodeUrl",
    label: "Encode URL",
    hint: "Auto-encode query values. Disable when values are pre-encoded.",
    defaultOn: true,
  },
  {
    key: "trimWhitespace",
    label: "Trim whitespace",
    hint: "Strip whitespace from headers, params, and body.",
    defaultOn: true,
  },
  {
    key: "historyDisabled",
    label: "Disable history",
    hint: "Stop logging runs in this block's history.",
    defaultOn: false,
  },
];

export function HttpSettingsDrawer({
  metadata,
  history,
  examples,
  settings,
  canSaveExample,
  onClose,
  onUpdateMetadata,
  onUpdateSettings,
  onDelete,
  onPurgeHistory,
  onSaveExample,
  onRestoreExample,
  onDeleteExample,
}: HttpSettingsDrawerProps) {
  return (
    <Portal>
      <Box
        position="fixed"
        top={0}
        right={0}
        bottom={0}
        w="320px"
        bg="bg.panel"
        borderLeftWidth="1px"
        borderColor="border"
        boxShadow="lg"
        zIndex={1500}
        overflowY="auto"
      >
        <Flex
          align="center"
          justify="space-between"
          px={4}
          py={3}
          borderBottomWidth="1px"
          borderColor="border.muted"
        >
          <Text fontSize="sm" fontWeight="semibold">
            HTTP block settings
          </Text>
          <IconButton
            aria-label="Close settings"
            size="xs"
            variant="ghost"
            onClick={onClose}
          >
            <LuX />
          </IconButton>
        </Flex>

        <Box px={4} py={3}>
          <Text
            fontSize="xs"
            color="fg.muted"
            textTransform="uppercase"
            letterSpacing="wide"
            mb={2}
          >
            Identity
          </Text>
          <Field.Root mb={3}>
            <Field.Label fontSize="xs">Alias</Field.Label>
            <Input
              size="sm"
              value={metadata.alias ?? ""}
              placeholder="e.g. createUser"
              onChange={(e) =>
                onUpdateMetadata({ alias: e.target.value || undefined })
              }
            />
          </Field.Root>
          <Field.Root mb={3}>
            <Field.Label fontSize="xs">Display</Field.Label>
            <NativeSelectRoot size="sm">
              <NativeSelectField
                value={metadata.displayMode ?? "input"}
                onChange={(e) =>
                  onUpdateMetadata({
                    displayMode: e.target.value as HttpDisplayMode,
                  })
                }
              >
                <option value="input">input</option>
                <option value="split">split</option>
                <option value="output">output</option>
              </NativeSelectField>
            </NativeSelectRoot>
          </Field.Root>

          <Text
            fontSize="xs"
            color="fg.muted"
            textTransform="uppercase"
            letterSpacing="wide"
            mt={4}
            mb={2}
          >
            Settings
          </Text>
          <Field.Root mb={3}>
            <Field.Label fontSize="xs">Timeout (ms)</Field.Label>
            <Input
              size="sm"
              type="number"
              value={metadata.timeoutMs ?? ""}
              placeholder="30000"
              onChange={(e) => {
                const v = e.target.value.trim();
                if (v === "") {
                  onUpdateMetadata({ timeoutMs: undefined });
                  return;
                }
                const n = Number(v);
                if (Number.isFinite(n) && n >= 0) {
                  onUpdateMetadata({ timeoutMs: Math.trunc(n) });
                }
              }}
            />
          </Field.Root>

          {/* ── Per-block flags (Onda 1) ── */}
          {PER_BLOCK_FLAGS.map(({ key, label, hint, defaultOn }) => {
            const value = settings[key];
            // `historyDisabled` defaults OFF (i.e. checked=false), all others
            // default ON. `value === undefined` means "use default".
            const checked = value === undefined ? defaultOn : value;
            return (
              <Flex
                key={key}
                align="center"
                justify="space-between"
                gap={2}
                mb={2}
              >
                <Box>
                  <Text fontSize="xs" fontWeight="medium">
                    {label}
                  </Text>
                  <Text fontSize="2xs" color="fg.muted">
                    {hint}
                  </Text>
                </Box>
                <Switch
                  size="sm"
                  checked={checked as boolean}
                  onCheckedChange={(e: { checked: boolean }) =>
                    onUpdateSettings({ [key]: e.checked })
                  }
                  aria-label={label}
                />
              </Flex>
            );
          })}

          <Text
            fontSize="xs"
            color="fg.muted"
            textTransform="uppercase"
            letterSpacing="wide"
            mt={4}
            mb={2}
          >
            History (last {history.length})
          </Text>
          {!metadata.alias ? (
            <Text fontSize="xs" color="fg.subtle">
              Set an alias to start tracking run history.
            </Text>
          ) : history.length === 0 ? (
            <Text fontSize="xs" color="fg.subtle">
              No runs yet.
            </Text>
          ) : (
            <>
              <Box>
                {history.map((entry) => {
                  const dot =
                    entry.outcome === "success" && entry.status
                      ? statusDotColor(entry.status)
                      : entry.outcome === "cancelled"
                        ? "gray.400"
                        : "red.500";
                  return (
                    <Flex
                      key={entry.id}
                      align="center"
                      gap={2}
                      py={1}
                      fontSize="xs"
                      fontFamily="mono"
                      color="fg.muted"
                      borderBottomWidth="1px"
                      borderColor="border.muted"
                      _last={{ borderBottomWidth: 0 }}
                    >
                      <Box w={1.5} h={1.5} borderRadius="full" bg={dot} />
                      <Text>{entry.method}</Text>
                      <Text>{entry.status ?? "—"}</Text>
                      <Text>{entry.elapsed_ms ?? 0}ms</Text>
                      <Box flex={1} />
                      <Text color="fg.subtle">
                        {relativeTimeAgo(new Date(entry.ran_at)) ?? ""}
                      </Text>
                    </Flex>
                  );
                })}
              </Box>
              <Box mt={2}>
                <Button size="2xs" variant="ghost" onClick={onPurgeHistory}>
                  Clear history
                </Button>
              </Box>
            </>
          )}

          {/* ── Examples (Onda 3) ── */}
          <Text
            fontSize="xs"
            color="fg.muted"
            textTransform="uppercase"
            letterSpacing="wide"
            mt={4}
            mb={2}
          >
            Examples ({examples.length})
          </Text>
          {!metadata.alias ? (
            <Text fontSize="xs" color="fg.subtle">
              Set an alias to pin response examples.
            </Text>
          ) : (
            <>
              {examples.length > 0 && (
                <Box>
                  {examples.map((ex) => (
                    <Flex
                      key={ex.id}
                      align="center"
                      gap={2}
                      py={1}
                      fontSize="xs"
                      borderBottomWidth="1px"
                      borderColor="border.muted"
                      _last={{ borderBottomWidth: 0 }}
                    >
                      <Box
                        as="button"
                        flex={1}
                        textAlign="left"
                        onClick={() => onRestoreExample(ex)}
                        _hover={{ color: "fg" }}
                        color="fg.muted"
                      >
                        <Text fontFamily="mono" truncate>
                          {ex.name}
                        </Text>
                        <Text fontSize="2xs" color="fg.subtle">
                          {relativeTimeAgo(new Date(ex.saved_at)) ?? ""}
                        </Text>
                      </Box>
                      <IconButton
                        aria-label={`Delete example ${ex.name}`}
                        size="2xs"
                        variant="ghost"
                        onClick={() => onDeleteExample(ex.id)}
                      >
                        <LuX />
                      </IconButton>
                    </Flex>
                  ))}
                </Box>
              )}
              <Box mt={2}>
                <Button
                  size="2xs"
                  variant="ghost"
                  disabled={!canSaveExample}
                  onClick={() => {
                    const name = window.prompt(
                      "Name this example (e.g. 'happy path 200'):",
                    );
                    if (name && name.trim()) onSaveExample(name.trim());
                  }}
                >
                  + Pin current response
                </Button>
                {!canSaveExample && (
                  <Text fontSize="2xs" color="fg.subtle" mt={1}>
                    Run the request first to pin a response.
                  </Text>
                )}
              </Box>
            </>
          )}

          <Box mt={6} pt={4} borderTopWidth="1px" borderColor="border.muted">
            <Button
              size="sm"
              colorPalette="red"
              variant="outline"
              w="full"
              onClick={onDelete}
            >
              <LuTrash2 /> Delete block
            </Button>
          </Box>
        </Box>
      </Box>
    </Portal>
  );
}
