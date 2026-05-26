
import { Box, Flex, HStack, Portal, Text } from "@chakra-ui/react";
import { useEffect, useRef, useState } from "react";

import { Btn, Input } from "@/components/atoms";
import { useEscapeClose } from "@/hooks/useEscapeClose";
import { useInlineForm } from "@/hooks/useInlineForm";
import { getActiveEditor } from "@/lib/codemirror/active-editor";
import { useEnvironmentStore } from "@/stores/environment";
import { useNewVariablePopoverStore } from "@/stores/newVariablePopover";

import { validateVariableName } from "./variable-name";

type VarType = "Text" | "Number" | "Bool" | "Secret";
const TYPES: VarType[] = ["Text", "Number", "Bool", "Secret"];

const HELPERS = [
  "{{uuid()}}",
  "{{now()}}",
  "{{base64()}}",
  "{{env()}}",
  "{{$prev.body.id}}",
];

export function NewVariablePopover() {
  const open = useNewVariablePopoverStore((s) => s.open);
  const close = useNewVariablePopoverStore((s) => s.closeForm);
  if (!open) return null;
  // NOT Dialog — no focus trap; return focus to CM6 on close.
  const handleClose = () => {
    close();
    getActiveEditor()?.focus();
  };
  return <NewVariableForm onClose={handleClose} />;
}

function NewVariableForm({ onClose }: { onClose: () => void }) {
  const activeEnv = useEnvironmentStore((s) => s.activeEnvironment);
  const setVariable = useEnvironmentStore((s) => s.setVariable);

  // F2: route the name through the shared inline-form idiom + the
  // real `validateVariableName` (was an ad-hoc `name.trim() === ""`
  // that skipped the whitespace/dot rules every other form enforces —
  // audit 05 Part B). No duplicate list is passed: this popover is an
  // upsert into the active env, so a colliding name is intentionally
  // allowed (behavior preserved). `ipcError` is the separate
  // save-failure channel.
  const nameField = useInlineForm("", validateVariableName);
  const [value, setValue] = useState("");
  const [type, setType] = useState<VarType>("Text");
  const [ipcError, setIpcError] = useState<string | null>(null);
  const cardRef = useRef<HTMLDivElement | null>(null);

  useEscapeClose(onClose);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (cardRef.current && !cardRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // Defer so the opening click doesn't immediately self-close.
    const t = setTimeout(
      () => document.addEventListener("mousedown", onDown, true),
      0,
    );
    return () => {
      clearTimeout(t);
      document.removeEventListener("mousedown", onDown, true);
    };
  }, [onClose]);

  async function handleSave() {
    if (!nameField.attemptSubmit()) return;
    if (!activeEnv) {
      setIpcError("No active environment");
      return;
    }
    try {
      await setVariable(
        activeEnv.id,
        nameField.value.trim(),
        value,
        type === "Secret",
      );
      onClose();
    } catch (e) {
      setIpcError(e instanceof Error ? e.message : "Failed to save");
    }
  }

  return (
    <Portal>
      <Box
        position="fixed"
        inset={0}
        zIndex={1500}
        display="flex"
        alignItems="flex-start"
        justifyContent="center"
        pt="14vh"
        bg="blackAlpha.300"
      >
        <Box
          ref={cardRef}
          data-testid="new-variable-popover"
          w="480px"
          maxW="92vw"
          bg="bg"
          borderWidth="1px"
          borderColor="border"
          borderRadius="8px"
          shadow="2xl"
          p={4}
        >
          <Text fontFamily="serif" fontSize="16px" fontWeight={500} mb={1}>
            New variable
          </Text>
          <Text fontSize="11px" color="fg.muted" mb={3}>
            {activeEnv
              ? `into ${activeEnv.name}`
              : "no active environment — pick one first"}
          </Text>

          <Input
            data-testid="new-variable-name"
            placeholder="VARIABLE_NAME"
            value={nameField.value}
            autoFocus
            aria-invalid={nameField.showError}
            onChange={(e) => nameField.setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleSave();
              }
            }}
          />

          <HStack gap={1} mt={3} mb={2}>
            {TYPES.map((t) => (
              <Btn
                key={t}
                variant={t === type ? "primary" : "ghost"}
                data-testid={`new-variable-type-${t}`}
                onClick={() => setType(t)}
              >
                {t}
              </Btn>
            ))}
          </HStack>

          <Input
            data-testid="new-variable-value"
            placeholder="value"
            type={type === "Secret" ? "password" : "text"}
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleSave();
              }
            }}
          />

          <TemplateHelpers onInsert={(t) => setValue((v) => v + t)} />

          {(nameField.showError || ipcError) && (
            <Text
              fontSize="11px"
              color="error"
              mt={2}
              data-testid="new-variable-error"
            >
              {nameField.showError ? nameField.error : ipcError}
            </Text>
          )}

          <Flex justify="flex-end" gap={2} mt={4}>
            <Btn
              variant="ghost"
              data-testid="new-variable-cancel"
              onClick={onClose}
            >
              Cancel
            </Btn>
            <Btn
              variant="primary"
              data-testid="new-variable-save"
              onClick={() => void handleSave()}
              disabled={!activeEnv || nameField.value.trim() === ""}
            >
              Save
            </Btn>
          </Flex>
        </Box>
      </Box>
    </Portal>
  );
}

function TemplateHelpers({ onInsert }: { onInsert: (t: string) => void }) {
  return (
    <Box mt={2}>
      <Text fontSize="10px" color="fg.subtle" mb={1}>
        Insert helper
      </Text>
      <Flex gap={1} wrap="wrap">
        {HELPERS.map((h) => (
          <Btn
            key={h}
            variant="ghost"
            data-testid={`new-variable-helper-${h}`}
            onClick={() => onInsert(h)}
          >
            {h}
          </Btn>
        ))}
      </Flex>
    </Box>
  );
}
