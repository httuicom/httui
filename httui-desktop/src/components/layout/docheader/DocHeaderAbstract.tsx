import { useEffect, useRef, useState } from "react";
import { Box, Text, chakra } from "@chakra-ui/react";

import {
  deriveAbstractDisplay,
  type DocHeaderFrontmatter,
} from "./docheader-derive";

const ABSTRACT_SAVE_DEBOUNCE_MS = 400;

export interface DocHeaderAbstractProps {
  frontmatter: DocHeaderFrontmatter | null;
  /** Number of lines to show in the collapsed state. Default 3
   *  matches the canvas spec. */
  collapsedLines?: number;
  /** Notion-mode editor — when provided, the abstract renders as an
   *  inline textarea. Debounced 400ms before firing. */
  onAbstractSave?: (abstract: string) => void;
}

export function DocHeaderAbstract({
  frontmatter,
  collapsedLines = 3,
  onAbstractSave,
}: DocHeaderAbstractProps) {
  const [expanded, setExpanded] = useState(false);

  if (onAbstractSave) {
    return (
      <DocHeaderAbstractInput
        value={frontmatter?.abstract?.trim() ?? ""}
        onSave={onAbstractSave}
      />
    );
  }

  const display = deriveAbstractDisplay(frontmatter);
  if (!display) return null;

  const showToggle = display.needsTruncation;
  const clamped = showToggle && !expanded;

  return (
    <Box
      data-testid="docheader-abstract"
      data-clamped={clamped || undefined}
      data-needs-truncation={display.needsTruncation || undefined}
      mt={3}
      position="relative"
    >
      <Text
        data-testid="docheader-abstract-text"
        as="p"
        fontFamily="serif"
        fontStyle="italic"
        fontSize="14px"
        lineHeight="1.5"
        color="fg.muted"
        m={0}
        css={
          clamped
            ? {
                display: "-webkit-box",
                WebkitLineClamp: collapsedLines,
                WebkitBoxOrient: "vertical",
                overflow: "hidden",
              }
            : undefined
        }
      >
        {display.text}
      </Text>
      {clamped && <FadeMask />}
      {showToggle && (
        <Text
          as="button"
          data-testid="docheader-abstract-toggle"
          fontFamily="mono"
          fontSize="11px"
          color="brand.fg"
          mt={1}
          onClick={() => setExpanded((v) => !v)}
          cursor="pointer"
          textAlign="left"
        >
          {expanded ? "less" : "more"}
        </Text>
      )}
    </Box>
  );
}

function FadeMask() {
  // Soft gradient overlay at the bottom of the clamped text. The
  // bg matches the card's bg.1 token so the mask blends with the
  // surrounding surface.
  return (
    <Box
      data-testid="docheader-abstract-fade"
      position="absolute"
      pointerEvents="none"
      bottom={0}
      left={0}
      right={0}
      h="2em"
      bgGradient="linear(to-b, transparent, var(--chakra-colors-bg-1))"
    />
  );
}

interface DocHeaderAbstractInputProps {
  value: string;
  onSave: (abstract: string) => void;
}

function DocHeaderAbstractInput({
  value,
  onSave,
}: DocHeaderAbstractInputProps) {
  const [local, setLocal] = useState(value);
  const lastExternalRef = useRef(value);

  useEffect(() => {
    if (value !== lastExternalRef.current && value !== local) {
      lastExternalRef.current = value;
      setLocal(value);
    } else {
      lastExternalRef.current = value;
    }
  }, [value, local]);

  const onSaveRef = useRef(onSave);
  useEffect(() => {
    onSaveRef.current = onSave;
  });

  useEffect(() => {
    if (local === value) return;
    const timer = setTimeout(() => {
      onSaveRef.current(local);
    }, ABSTRACT_SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [local, value]);

  // Single-line semantics: pressing Enter shouldn't insert a newline
  // (the slice-1 schema can't round-trip them). It commits-by-blur
  // instead so the user-visible flow matches the title field.
  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      e.currentTarget.blur();
    }
  };

  return (
    <Box mt={3} position="relative">
      <chakra.textarea
        data-testid="docheader-abstract-input"
        rows={2}
        value={local}
        onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
          setLocal(e.target.value)
        }
        onKeyDown={onKeyDown}
        placeholder="Add a description…"
        fontFamily="serif"
        fontStyle="italic"
        fontSize="14px"
        lineHeight="1.5"
        color="fg.muted"
        bg="transparent"
        border="none"
        outline="none"
        resize="none"
        width="100%"
        m={0}
        p={0}
        css={{
          "&::placeholder": {
            color: "var(--chakra-colors-fg-3)",
            opacity: 1,
          },
        }}
      />
    </Box>
  );
}
