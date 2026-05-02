// Epic 50 Story 01 + 02 — DocHeader card scaffold.
//
// Pure presentational. Renders above the CM6 editor for `.md` tabs.
// Story 03 (meta strip) + Story 04 (abstract paragraph) + Story 05
// (action row) extend the card; Story 06 (compact mode) flips a
// data attribute. The frontmatter parser is Epic 52 — this card
// accepts already-parsed `frontmatter` and `firstHeading` props.
//
// V2 / cenário 4.5 / M2 — when `onTitleSave` is provided the H1
// becomes an editable input (Notion-mode), debounced 300ms before
// firing the callback. Static H1 path is preserved for callers that
// don't pass `onTitleSave` (kept the diff viewer + tests working).

import { useEffect, useRef, useState } from "react";
import { Box, Flex, Heading, Text } from "@chakra-ui/react";

import {
  deriveBreadcrumb,
  pickH1Title,
  type DocHeaderFrontmatter,
} from "./docheader-derive";

const TITLE_SAVE_DEBOUNCE_MS = 300;

export interface DocHeaderCardProps {
  filePath: string;
  /** Vault-relative path; the breadcrumb is derived from it. When
   *  unset, the breadcrumb is hidden. */
  relativeFilePath?: string | null;
  frontmatter?: DocHeaderFrontmatter | null;
  firstHeading?: string | null;
  /** Story 06 — compact mode hides everything below the meta strip.
   *  Story 03 ships the meta strip; until then `compact === true`
   *  hides nothing visible. */
  compact?: boolean;
  /** Click handler for breadcrumb segments. The leaf is rendered as
   *  inactive even when `onBreadcrumbSelect` is provided. */
  onBreadcrumbSelect?: (path: string) => void;
  /** Click handler for the H1 — Story 06 uses it to toggle compact
   *  mode. Ignored when `onTitleSave` is provided (the editable input
   *  takes over the click target). */
  onTitleClick?: () => void;
  /** When provided, the title renders as an editable input (Notion-
   *  mode). The callback fires 300ms after the last keystroke with the
   *  new value (trimmed). Empty values are filtered out by the
   *  consumer's `updateFrontmatterTitle` helper. */
  onTitleSave?: (title: string) => void;
}

export function DocHeaderCard({
  filePath,
  relativeFilePath,
  frontmatter,
  firstHeading,
  compact,
  onBreadcrumbSelect,
  onTitleClick,
  onTitleSave,
}: DocHeaderCardProps) {
  const editable = onTitleSave !== undefined;
  // For the editable surface we only honor frontmatter.title — falling
  // back to the filename would write the filename to disk on first
  // commit, breaking the virtual-mode contract. The placeholder shows
  // when the user hasn't typed a title yet.
  const editableValue = frontmatter?.title?.trim() ?? "";
  const staticTitle = pickH1Title(
    frontmatter ?? null,
    firstHeading ?? null,
    filePath,
  );
  const breadcrumb = relativeFilePath
    ? deriveBreadcrumb(relativeFilePath)
    : [];

  return (
    <Box
      data-testid="docheader-card"
      data-compact={compact || undefined}
      px={6}
      py={5}
      borderBottomWidth="1px"
      borderBottomColor="line"
      bg="bg.1"
    >
      {breadcrumb.length > 1 && (
        <Flex
          data-testid="docheader-breadcrumb"
          gap={1}
          align="center"
          mb={2}
          flexWrap="wrap"
        >
          {breadcrumb.map((seg, i) => {
            const isLeaf = i === breadcrumb.length - 1;
            return (
              <Flex
                key={seg.path}
                align="center"
                gap={1}
                data-testid={`docheader-breadcrumb-segment-${i}`}
              >
                {i > 0 && <BreadcrumbSeparator />}
                <Text
                  as={onBreadcrumbSelect && !isLeaf ? "button" : "span"}
                  data-leaf={isLeaf || undefined}
                  fontFamily="mono"
                  fontSize="11px"
                  color={isLeaf ? "fg.2" : "fg.3"}
                  cursor={
                    onBreadcrumbSelect && !isLeaf ? "pointer" : undefined
                  }
                  onClick={
                    onBreadcrumbSelect && !isLeaf
                      ? () => onBreadcrumbSelect(seg.path)
                      : undefined
                  }
                  _hover={
                    onBreadcrumbSelect && !isLeaf
                      ? { color: "fg" }
                      : undefined
                  }
                >
                  {seg.label}
                </Text>
              </Flex>
            );
          })}
        </Flex>
      )}

      {editable ? (
        <DocHeaderTitleInput
          value={editableValue}
          onSave={onTitleSave!}
        />
      ) : (
        <Heading
          as={onTitleClick ? "button" : "h1"}
          data-testid="docheader-title"
          fontFamily="serif"
          fontSize="2.25rem"
          fontWeight={600}
          color="fg"
          textAlign="left"
          cursor={onTitleClick ? "pointer" : undefined}
          onClick={onTitleClick}
          m={0}
        >
          {staticTitle}
        </Heading>
      )}
    </Box>
  );
}

interface DocHeaderTitleInputProps {
  value: string;
  onSave: (title: string) => void;
}

function DocHeaderTitleInput({ value, onSave }: DocHeaderTitleInputProps) {
  const [local, setLocal] = useState(value);
  // Track the last `value` we sync'd from the parent so we can
  // detect a real external change without re-triggering on our own
  // commits (which round-trip through the parent and arrive as a new
  // value identical to the local state).
  const lastExternalRef = useRef(value);
  // Sync external changes (loaded a new file, undo/redo in body, etc.)
  useEffect(() => {
    if (value !== lastExternalRef.current && value !== local) {
      lastExternalRef.current = value;
      setLocal(value);
    } else {
      lastExternalRef.current = value;
    }
  }, [value, local]);

  // Keep onSave in a ref so the debounce effect doesn't re-trigger
  // when the parent rebuilds the callback (DocHeaderedEditor recreates
  // it on every body keystroke since `content` is in the deps).
  const onSaveRef = useRef(onSave);
  useEffect(() => {
    onSaveRef.current = onSave;
  });

  useEffect(() => {
    if (local === value) return;
    const timer = setTimeout(() => {
      onSaveRef.current(local);
    }, TITLE_SAVE_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [local, value]);

  return (
    <Box
      as="input"
      data-testid="docheader-title"
      type="text"
      value={local}
      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
        setLocal(e.target.value)
      }
      placeholder="Untitled"
      fontFamily="serif"
      fontSize="2.25rem"
      fontWeight={600}
      color="fg"
      bg="transparent"
      border="none"
      outline="none"
      width="100%"
      m={0}
      p={0}
      _placeholder={{ color: "fg.3" }}
      // Cursor stays the default text caret; remove any hover styling
      // inherited from elsewhere.
    />
  );
}

function BreadcrumbSeparator() {
  return (
    <Text as="span" fontFamily="mono" fontSize="11px" color="fg.3">
      /
    </Text>
  );
}
