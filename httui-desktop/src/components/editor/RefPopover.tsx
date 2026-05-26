// Body of the inline `{{ref}}` quick popover.
//
// Visual contract: design-canvas §4.3 PopoverVarQuick — 10px radius,
// layered shadow, uppercase section labels, the key echoed in the
// same violet as the in-editor chip (continuity), per-env value in a
// mono field, dashed session-override input. Mounted inside Chakra's
// Popover (positioning/arrow owned by the host). Reuses V5
// useSessionOverrideStore + TemporaryChip. Block-ref shaped keys
// (with dots) get a read-only note.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useEffect, useState } from "react";
import { LuChevronDown, LuChevronUp } from "react-icons/lu";

import { Btn, Input } from "@/components/atoms";
import { TemporaryChip } from "@/components/layout/variables/TemporaryChip";
import { grepVarUses, type VarUseEntry } from "@/lib/tauri/var-uses";
import { useEnvironmentStore } from "@/stores/environment";
import { useSessionOverrideStore } from "@/stores/sessionOverride";
import type { RefPopoverState } from "@/lib/blocks/cm-ref-popover";

export interface RefPopoverProps {
  state: RefPopoverState;
  vaultPath: string | null;
  onClose: () => void;
}

// Matches the `.cm-reference-highlight` decoration so the popover
// reads as "this is that chip" (cm-references.ts uses the same hue).
const REF_VIOLET = "rgb(139, 92, 246)";
const REF_VIOLET_BG = "rgba(139, 92, 246, 0.14)";

const POPOVER_SHADOW =
  "0 24px 60px -16px rgba(30,41,59,0.35), 0 4px 12px -4px rgba(30,41,59,0.22)";

function SectionLabel({ children }: { children: string }) {
  return (
    <Text
      fontSize="10px"
      fontWeight={600}
      textTransform="uppercase"
      letterSpacing="0.06em"
      color="fg.subtle"
    >
      {children}
    </Text>
  );
}

export function RefPopover({ state, vaultPath, onClose }: RefPopoverProps) {
  const rawKey = state.rawKey;
  const isEnvVar = !rawKey.includes(".");
  const envName = useEnvironmentStore((s) => s.activeEnvironment?.name ?? null);
  const clearOverride = useSessionOverrideStore((s) => s.clearOverride);
  const override = useSessionOverrideStore((s) =>
    envName ? s.overrides[envName]?.[rawKey] : undefined,
  );

  return (
    <Box
      data-testid="ref-popover"
      data-ref={rawKey}
      position="relative"
      bg="bg"
      borderWidth="1px"
      borderColor="border"
      borderRadius="10px"
      boxShadow={POPOVER_SHADOW}
      minW="320px"
      maxW="420px"
    >
      {/* header */}
      <Flex
        align="center"
        gap={2}
        px={4}
        py={3}
        borderBottomWidth="1px"
        borderBottomColor="border"
      >
        <Box
          as="span"
          fontFamily="mono"
          fontSize="13px"
          fontWeight={600}
          color={REF_VIOLET}
          bg={REF_VIOLET_BG}
          px={2}
          py={0.5}
          borderRadius="5px"
          maxW="220px"
          truncate
        >
          {`{{${rawKey}}}`}
        </Box>
        {override !== undefined && envName && (
          <TemporaryChip onClear={() => clearOverride(envName, rawKey)} />
        )}
      </Flex>

      <Box px={4} py={3}>
        {!isEnvVar ? (
          <Flex
            gap={2}
            align="flex-start"
            bg="bg.subtle"
            borderRadius="6px"
            px={3}
            py={2}
            data-testid="ref-popover-blockref"
          >
            <Text fontSize="11px" color="fg.muted">
              Block reference — resolves from the block above at run time.
            </Text>
          </Flex>
        ) : (
          <RefEnvVarBody
            rawKey={rawKey}
            envName={envName}
            override={override}
            vaultPath={vaultPath}
          />
        )}
      </Box>

      {/* footer */}
      <Flex
        align="center"
        justify="flex-end"
        px={4}
        py={2.5}
        borderTopWidth="1px"
        borderTopColor="border"
      >
        <Btn variant="ghost" data-testid="ref-popover-close" onClick={onClose}>
          Close{" "}
          <Box
            as="span"
            ml={2}
            fontFamily="mono"
            fontSize="9px"
            color="fg.subtle"
            borderWidth="1px"
            borderColor="border"
            borderRadius="3px"
            px={1}
          >
            esc
          </Box>
        </Btn>
      </Flex>
    </Box>
  );
}

interface RefEnvVarBodyProps {
  rawKey: string;
  envName: string | null;
  override: string | undefined;
  vaultPath: string | null;
}

function RefEnvVarBody({
  rawKey,
  envName,
  override,
  vaultPath,
}: RefEnvVarBodyProps) {
  const setOverride = useSessionOverrideStore((s) => s.setOverride);
  const [resolved, setResolved] = useState<string | null>(null);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let alive = true;
    void Promise.resolve(
      useEnvironmentStore.getState().getActiveVariables(),
    ).then((vars) => {
      if (alive) setResolved(vars[rawKey] ?? null);
    });
    return () => {
      alive = false;
    };
  }, [rawKey]);

  function applyOverride() {
    if (!envName || draft.trim() === "") return;
    setOverride(envName, rawKey, draft);
    setDraft("");
  }

  const hasOverride = override !== undefined;

  return (
    <Flex direction="column" gap={3}>
      <Box>
        <SectionLabel>
          {envName ? `value in ${envName}` : "no active environment"}
        </SectionLabel>
        <Flex
          mt={1}
          align="center"
          gap={2}
          bg="bg.subtle"
          borderWidth="1px"
          borderColor={hasOverride ? REF_VIOLET : "border"}
          borderRadius="6px"
          px={2.5}
          py={1.5}
        >
          <Text
            flex={1}
            fontFamily="mono"
            fontSize="12px"
            color={resolved == null && !hasOverride ? "fg.subtle" : "fg"}
            data-testid="ref-popover-value"
            wordBreak="break-all"
          >
            {override ?? resolved ?? "(not set in active env)"}
          </Text>
          {hasOverride && (
            <Box
              as="span"
              fontFamily="serif"
              fontStyle="italic"
              fontSize="9px"
              color={REF_VIOLET}
              flexShrink={0}
            >
              overridden
            </Box>
          )}
        </Flex>
      </Box>

      <Box>
        <SectionLabel>session override</SectionLabel>
        <Flex gap={2} mt={1}>
          <Input
            data-testid="ref-popover-override-input"
            placeholder="temporary value…"
            value={draft}
            borderStyle="dashed"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                applyOverride();
              }
            }}
            disabled={!envName}
          />
          <Btn
            variant="primary"
            data-testid="ref-popover-override-set"
            onClick={applyOverride}
            disabled={!envName || draft.trim() === ""}
          >
            Set
          </Btn>
        </Flex>
        <Text fontSize="10px" color="fg.subtle" mt={1} fontStyle="italic">
          reverts to the stored value when the app restarts
        </Text>
      </Box>

      <RefUsesSection rawKey={rawKey} vaultPath={vaultPath} />
    </Flex>
  );
}

function RefUsesSection({
  rawKey,
  vaultPath,
}: {
  rawKey: string;
  vaultPath: string | null;
}) {
  const [uses, setUses] = useState<VarUseEntry[] | null>(null);
  const [showUses, setShowUses] = useState(false);

  useEffect(() => {
    if (!vaultPath) return;
    let alive = true;
    grepVarUses(vaultPath, rawKey)
      .then((u) => alive && setUses(u))
      .catch(() => alive && setUses([]));
    return () => {
      alive = false;
    };
  }, [vaultPath, rawKey]);

  const count = uses?.length ?? 0;

  return (
    <Box borderTopWidth="1px" borderTopColor="border" pt={2}>
      <Btn
        variant="ghost"
        data-testid="ref-popover-uses"
        onClick={() => setShowUses((v) => !v)}
        disabled={count === 0}
      >
        {count === 0
          ? "Not used in any block yet"
          : `Used in ${count} block${count === 1 ? "" : "s"}`}
        {count > 0 && (
          <Box as="span" ml={2} color="fg.subtle" display="inline-flex">
            {showUses ? <LuChevronUp size={12} /> : <LuChevronDown size={12} />}
          </Box>
        )}
      </Btn>
      {showUses && count > 0 && (
        <Box
          mt={2}
          maxH="140px"
          overflowY="auto"
          bg="bg.subtle"
          borderRadius="6px"
          px={2.5}
          py={1.5}
        >
          {uses?.map((u, i) => (
            <Text
              key={`${u.file_path}:${u.line}:${i}`}
              fontFamily="mono"
              fontSize="10px"
              color="fg.muted"
              truncate
              data-testid="ref-popover-use-row"
            >
              {u.file_path}:{u.line}
            </Text>
          ))}
        </Box>
      )}
    </Box>
  );
}
