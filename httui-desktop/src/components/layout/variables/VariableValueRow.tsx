// Canvas §6 Variables — detail panel value row (Epic 43 Story 03).
//
// One row per env. Two top-level modes: VIEW (display + Show/Hide for
// secrets + Edit) and EDIT (input + Save/Cancel). Edit is gated to
// values the user can already see — non-secret rows or
// secret-revealed rows. When the parent passes `override`, the row
// flips to OVERRIDE mode: the override value renders cleartext + the
// TEMPORARY chip; reveal/edit are bypassed (clear the override first
// to edit the underlying). The consumer plugs `fetchSecret` for
// keychain resolution and `onCommit` to persist the edit
// (`EnvironmentsStore::set_var` lands at the page mount).

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";

import { Btn, Input } from "@/components/atoms";

import { TemporaryChip } from "./TemporaryChip";

const SECRET_MASK = "••••••••";

export interface VariableValueRowProps {
  env: string;
  /** Ground-truth value from `row.values[env]`. Undefined → em-dash. */
  value: string | undefined;
  isSecret: boolean;
  /** Async cleartext fetch (keychain). Returning undefined renders an empty cleartext. */
  fetchSecret?: (env: string) => Promise<string | undefined>;
  /** Called on Save with the new draft. Consumer wires the store/Tauri write. */
  onCommit?: (env: string, next: string) => void;
  /** Active session override for this env. When set, wins over `value` and
   * `fetchSecret` — the chip is shown and reveal/edit are bypassed. */
  override?: string;
  /** Click handler for the TEMPORARY chip. Required to make the chip interactive. */
  onClearOverride?: () => void;
}

type RevealState =
  | { kind: "masked" }
  | { kind: "loading" }
  | { kind: "revealed"; value: string }
  | { kind: "error"; message: string };

export function VariableValueRow({
  env,
  value,
  isSecret,
  fetchSecret,
  onCommit,
  override,
  onClearOverride,
}: VariableValueRowProps) {
  const [reveal, setReveal] = useState<RevealState>({ kind: "masked" });
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const isOverridden = override !== undefined;

  async function handleShow() {
    if (!fetchSecret) return;
    setReveal({ kind: "loading" });
    try {
      const v = await fetchSecret(env);
      setReveal({ kind: "revealed", value: v ?? "" });
    } catch (e) {
      setReveal({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }

  function handleHide() {
    setReveal({ kind: "masked" });
  }

  function handleEdit() {
    if (isSecret && reveal.kind === "revealed") {
      setDraft(reveal.value);
    } else if (!isSecret) {
      setDraft(value ?? "");
    } else {
      return;
    }
    setEditing(true);
  }

  function handleSave() {
    onCommit?.(env, draft);
    if (isSecret && reveal.kind === "revealed") {
      setReveal({ kind: "revealed", value: draft });
    }
    setEditing(false);
  }

  function handleCancel() {
    setEditing(false);
  }

  const canEdit = !isSecret || reveal.kind === "revealed";

  return (
    <Flex
      data-testid={`variable-value-row-${env}`}
      data-mode={editing ? "edit" : isOverridden ? "override" : "view"}
      align="center"
      gap={2}
      px={4}
      py={2}
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        w="68px"
        flexShrink={0}
        truncate
        data-testid={`variable-value-row-${env}-env-label`}
      >
        {env}
      </Text>
      {editing ? (
        <ValueEditor
          env={env}
          draft={draft}
          onChangeDraft={setDraft}
          onSave={handleSave}
          onCancel={handleCancel}
        />
      ) : isOverridden ? (
        <>
          <Box flex={1} minW={0}>
            <Text
              fontFamily="mono"
              fontSize="11px"
              color="fg"
              title={override}
              truncate
              data-testid={`variable-value-row-${env}-display`}
            >
              {override}
            </Text>
          </Box>
          <TemporaryChip onClear={onClearOverride} />
        </>
      ) : (
        <>
          <Box flex={1} minW={0}>
            <ValueDisplay
              env={env}
              value={value}
              isSecret={isSecret}
              reveal={reveal}
            />
          </Box>
          {isSecret && (
            <SecretToggle
              env={env}
              reveal={reveal}
              enabled={!!fetchSecret}
              onShow={handleShow}
              onHide={handleHide}
            />
          )}
          {canEdit && onCommit && (
            <Btn
              variant="ghost"
              data-testid={`variable-value-row-${env}-edit`}
              onClick={handleEdit}
            >
              Edit
            </Btn>
          )}
        </>
      )}
    </Flex>
  );
}

function ValueDisplay({
  env,
  value,
  isSecret,
  reveal,
}: {
  env: string;
  value: string | undefined;
  isSecret: boolean;
  reveal: RevealState;
}) {
  const testId = `variable-value-row-${env}-display`;

  if (reveal.kind === "loading") {
    return (
      <Text fontFamily="mono" fontSize="11px" color="fg.subtle" data-testid={testId}>
        carregando…
      </Text>
    );
  }
  if (reveal.kind === "error") {
    return (
      <Text
        fontFamily="mono"
        fontSize="11px"
        color="error"
        data-testid={testId}
        title={reveal.message}
      >
        ⚠ {reveal.message}
      </Text>
    );
  }
  if (isSecret && reveal.kind !== "revealed") {
    return (
      <Text fontFamily="mono" fontSize="11px" color="fg.muted" data-testid={testId}>
        {SECRET_MASK}
      </Text>
    );
  }
  if (isSecret && reveal.kind === "revealed") {
    return (
      <Text
        fontFamily="mono"
        fontSize="11px"
        color="fg"
        title={reveal.value}
        truncate
        data-testid={testId}
      >
        {reveal.value || (
          <Text as="span" color="fg.subtle">
            {"(vazio)"}
          </Text>
        )}
      </Text>
    );
  }
  if (value === undefined) {
    return (
      <Text fontFamily="mono" fontSize="11px" color="fg.subtle" data-testid={testId}>
        —
      </Text>
    );
  }
  return (
    <Text
      fontFamily="mono"
      fontSize="11px"
      color="fg"
      title={value}
      truncate
      data-testid={testId}
    >
      {value}
    </Text>
  );
}

function SecretToggle({
  env,
  reveal,
  enabled,
  onShow,
  onHide,
}: {
  env: string;
  reveal: RevealState;
  enabled: boolean;
  onShow: () => void;
  onHide: () => void;
}) {
  if (reveal.kind === "revealed") {
    return (
      <Btn
        variant="ghost"
        data-testid={`variable-value-row-${env}-hide`}
        onClick={onHide}
      >
        Hide
      </Btn>
    );
  }
  return (
    <Btn
      variant="ghost"
      data-testid={`variable-value-row-${env}-show`}
      onClick={onShow}
      disabled={!enabled || reveal.kind === "loading"}
    >
      {reveal.kind === "loading" ? "…" : "Show"}
    </Btn>
  );
}

function ValueEditor({
  env,
  draft,
  onChangeDraft,
  onSave,
  onCancel,
}: {
  env: string;
  draft: string;
  onChangeDraft: (v: string) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <>
      <Box flex={1} minW={0}>
        <Input
          data-testid={`variable-value-row-${env}-input`}
          value={draft}
          onChange={(e) => onChangeDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              onSave();
            } else if (e.key === "Escape") {
              e.preventDefault();
              onCancel();
            }
          }}
          autoFocus
        />
      </Box>
      <Btn
        variant="primary"
        data-testid={`variable-value-row-${env}-save`}
        onClick={onSave}
      >
        Save
      </Btn>
      <Btn
        variant="ghost"
        data-testid={`variable-value-row-${env}-cancel`}
        onClick={onCancel}
      >
        Cancel
      </Btn>
    </>
  );
}
