import { useEffect, useState } from "react";
import { Box, Flex, HStack, Stack, Text } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";

import type {
  PreflightCheck,
  PreflightCheckKind,
} from "@/lib/blocks/preflight-checks";

import { PreflightValueEditor } from "./PreflightValueEditor";

interface KindOption {
  kind: PreflightCheckKind;
  label: string;
  placeholder: string;
}

const KIND_OPTIONS: ReadonlyArray<KindOption> = [
  {
    kind: "connection",
    label: "connection",
    placeholder: "connection name (e.g., payments-db)",
  },
  {
    kind: "env_var",
    label: "env var",
    placeholder: "ENV VAR NAME",
  },
  {
    kind: "branch",
    label: "branch",
    placeholder: "branch name (e.g., main)",
  },
  {
    kind: "file_exists",
    label: "file exists",
    placeholder: "./path/to/file",
  },
  {
    kind: "command",
    label: "command",
    placeholder: "binary name (e.g., psql)",
  },
];

export interface PreflightCheckPopoverProps {
  /** Anchor rect — popover positions below this point. */
  anchorRect?: DOMRect | null;
  /** Edit mode: pre-bind kind + value. */
  initialKind?: PreflightCheckKind;
  initialValue?: string;
  onSave: (check: PreflightCheck) => void;
  /** Remove callback (edit mode only). */
  onRemove?: () => void;
  onClose: () => void;
  /** Autocomplete provider per kind. Tests inject deterministic data. */
  getSuggestions?: (kind: PreflightCheckKind) => Promise<string[]>;
}

export function PreflightCheckPopover({
  anchorRect,
  initialKind,
  initialValue,
  onSave,
  onRemove,
  onClose,
  getSuggestions,
}: PreflightCheckPopoverProps) {
  const [kind, setKind] = useState<PreflightCheckKind | null>(
    initialKind ?? null,
  );
  const [value, setValue] = useState(initialValue ?? "");

  // Once a kind is picked, the CM6 editor owns Esc (dismisses autocomplete first).
  useEffect(() => {
    if (kind !== null) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [kind, onClose]);

  const submit = () => {
    if (!kind) return;
    const trimmed = value.trim();
    if (trimmed.length === 0) return;
    onSave({ kind, value: trimmed });
  };

  // Falls back to centered when no anchor rect is supplied (test mounts).
  const position = anchorRect
    ? {
        top: `${anchorRect.bottom + 4}px`,
        left: `${anchorRect.left}px`,
      }
    : { top: "20%", left: "50%", transform: "translateX(-50%)" };

  return (
    <>
      <Box
        data-testid="preflight-check-popover-overlay"
        position="fixed"
        inset={0}
        bg="transparent"
        zIndex={9998}
        onClick={onClose}
      />
      <Box
        data-testid="preflight-check-popover"
        role="dialog"
        aria-modal="true"
        position="fixed"
        {...position}
        w="320px"
        bg="bg.subtle"
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        shadow="2xl"
        zIndex={9999}
        p={3}
      >
        <Text
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          textTransform="uppercase"
          letterSpacing="0.06em"
          mb={2}
        >
          {initialKind ? "edit pre-flight check" : "add pre-flight check"}
        </Text>

        {kind === null ? (
          <KindPicker onSelect={setKind} />
        ) : (
          <Stack gap={2}>
            <HStack gap={2}>
              <Text
                data-testid="preflight-check-popover-kind"
                fontFamily="mono"
                fontSize="11px"
                color="fg.muted"
                px={2}
                py={1}
                borderRadius="999px"
                bg="bg.muted"
                borderWidth="1px"
                borderColor="border"
              >
                {KIND_OPTIONS.find((opt) => opt.kind === kind)?.label ?? kind}
              </Text>
              {!initialKind && (
                <Btn
                  data-testid="preflight-check-popover-back"
                  variant="ghost"
                  onClick={() => setKind(null)}
                >
                  ← back
                </Btn>
              )}
            </HStack>
            <Box data-testid="preflight-check-popover-value">
              <PreflightValueEditor
                kind={kind}
                value={value}
                onChange={setValue}
                onCommit={submit}
                onCancel={onClose}
                getSuggestions={getSuggestions}
              />
            </Box>
            <Flex justify="space-between" align="center" gap={2}>
              {onRemove ? (
                <Btn
                  data-testid="preflight-check-popover-remove"
                  variant="ghost"
                  onClick={() => {
                    onRemove();
                  }}
                >
                  Remove
                </Btn>
              ) : (
                <Box />
              )}
              <HStack gap={2}>
                <Btn
                  data-testid="preflight-check-popover-cancel"
                  variant="ghost"
                  onClick={onClose}
                >
                  Cancel
                </Btn>
                <Btn
                  data-testid="preflight-check-popover-save"
                  variant="primary"
                  disabled={value.trim().length === 0}
                  onClick={submit}
                >
                  Save
                </Btn>
              </HStack>
            </Flex>
          </Stack>
        )}
      </Box>
    </>
  );
}

interface KindPickerProps {
  onSelect: (kind: PreflightCheckKind) => void;
}

function KindPicker({ onSelect }: KindPickerProps) {
  return (
    <Stack data-testid="preflight-check-popover-kind-picker" gap={1}>
      {KIND_OPTIONS.map((opt) => (
        <Box
          key={opt.kind}
          as="button"
          data-testid={`preflight-check-popover-kind-${opt.kind}`}
          textAlign="left"
          px={3}
          py={2}
          borderRadius="md"
          fontFamily="mono"
          fontSize="11px"
          color="fg"
          bg="transparent"
          _hover={{ bg: "bg.muted" }}
          onClick={() => onSelect(opt.kind)}
        >
          <Text as="span" color="brand.fg" mr={2}>
            →
          </Text>
          {opt.label}
        </Box>
      ))}
    </Stack>
  );
}
