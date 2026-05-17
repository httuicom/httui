// Block-status captures footer — Epic 46 Story 04.
//
// Compact `↗ N captured` summary that expands to a list of `key =
// value` rows. Secret-named values mask in the panel until the user
// clicks (clipboard copy still works on the masked row). Pure
// presentational consumer — the consumer feeds it the
// `Record<key, CaptureEntry>` from `useCaptureStore.getBlockCaptures`.

import { Box, Text, chakra } from "@chakra-ui/react";
import { useState } from "react";

import type { CaptureEntry } from "@/stores/captureStore";

const SECRET_MASK = "••••••••";
const VALUE_MAX_CHARS = 80;

export interface CapturesFooterProps {
  /** All captures for the current block. Empty object hides the footer. */
  captures: Readonly<Record<string, CaptureEntry>>;
  /** Per-row copy handler. When omitted, clicking a row is a no-op
   * (the consumer typically wires `navigator.clipboard.writeText`). */
  onCopy?: (key: string, value: string) => void;
  /** Initial open state for the expand panel. Default closed. */
  defaultOpen?: boolean;
}

export function CapturesFooter({
  captures,
  onCopy,
  defaultOpen = false,
}: CapturesFooterProps) {
  const entries = Object.entries(captures);
  const [open, setOpen] = useState(defaultOpen);

  if (entries.length === 0) return null;

  return (
    <Box
      data-testid="captures-footer"
      data-open={open || undefined}
      borderTopWidth="1px"
      borderTopColor="border"
      bg="bg.muted"
    >
      <chakra.button
        type="button"
        data-testid="captures-footer-summary"
        onClick={() => setOpen((v) => !v)}
        display="flex"
        alignItems="center"
        gap={2}
        px={4}
        py={2}
        bg="transparent"
        borderWidth={0}
        textAlign="left"
        w="full"
        cursor="pointer"
        _hover={{ bg: "bg.subtle" }}
      >
        <Text as="span" fontSize="13px" color="brand.fg" flexShrink={0}>
          ↗
        </Text>
        <Text
          fontFamily="mono"
          fontSize="11px"
          color="fg.muted"
          data-testid="captures-footer-summary-label"
        >
          {entries.length} captured
        </Text>
        <Box flex={1} />
        <Text
          as="span"
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          aria-hidden
        >
          {open ? "▾" : "▸"}
        </Text>
      </chakra.button>

      {open && (
        <Box data-testid="captures-footer-list">
          {entries.map(([key, entry]) => (
            <CaptureRow key={key} name={key} entry={entry} onCopy={onCopy} />
          ))}
        </Box>
      )}
    </Box>
  );
}

function CaptureRow({
  name,
  entry,
  onCopy,
}: {
  name: string;
  entry: CaptureEntry;
  onCopy?: (key: string, value: string) => void;
}) {
  const stringValue =
    entry.value === null
      ? ""
      : typeof entry.value === "string"
        ? entry.value
        : String(entry.value);

  const display = entry.isSecret
    ? SECRET_MASK
    : stringValue.length > VALUE_MAX_CHARS
      ? `${stringValue.slice(0, VALUE_MAX_CHARS)}…`
      : stringValue;

  const interactive = !!onCopy;

  const Comp = interactive ? chakra.button : chakra.div;
  return (
    <Comp
      type={interactive ? "button" : undefined}
      data-testid={`captures-footer-row-${name}`}
      data-secret={entry.isSecret || undefined}
      onClick={interactive ? () => onCopy?.(name, stringValue) : undefined}
      display="flex"
      alignItems="baseline"
      gap={2}
      px={4}
      py={1}
      bg="transparent"
      borderWidth={0}
      textAlign="left"
      w="full"
      cursor={interactive ? "pointer" : "default"}
      title={interactive ? "Click to copy" : undefined}
      _hover={interactive ? { bg: "bg.subtle" } : undefined}
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        flexShrink={0}
        truncate
        maxW="160px"
      >
        {name}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        flexShrink={0}
      >
        =
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color={entry.isSecret ? "fg.muted" : "fg"}
        truncate
        title={entry.isSecret ? "secret value" : stringValue}
      >
        {display}
      </Text>
      {entry.isSecret && (
        <Text
          as="span"
          fontSize="9px"
          color="fg.subtle"
          flexShrink={0}
          data-testid={`captures-footer-row-${name}-secret-chip`}
        >
          🔒
        </Text>
      )}
    </Comp>
  );
}
