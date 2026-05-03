// V6 / cenário 5 — colored tag dots rendered on the right side of
// `.md` rows in the file tree. Reads from `useTagIndexStore.byFile`
// (kept in sync per-save by `useEditorSession.refreshTagsForFile` and
// at vault open by `useTagIndexStore.loadFromVault`).
//
// Limit of 3 dots per row keeps the right rail compact even on
// notes with a long `tags:` list. The color palette is small + stable
// (hash → index) so the same tag always lands on the same swatch
// regardless of which file it shows up on.

import { Box, HStack } from "@chakra-ui/react";

import { useTagIndexStore } from "@/stores/tagIndex";

const MAX_DOTS = 3;

// Six muted swatches that read well in both light and dark mode.
// Tokens come from Chakra semantic colors so theme switches stay live.
const PALETTE = [
  "blue.solid",
  "purple.solid",
  "teal.solid",
  "orange.solid",
  "pink.solid",
  "green.solid",
] as const;

interface FileTreeTagDotsProps {
  filePath: string;
}

export function FileTreeTagDots({ filePath }: FileTreeTagDotsProps) {
  const tags = useTagIndexStore((s) => s.byFile[filePath]);
  if (!tags || tags.length === 0) return null;

  const visible = tags.slice(0, MAX_DOTS);
  const overflow = tags.length - visible.length;

  return (
    <HStack
      data-testid="file-tree-tag-dots"
      gap={0.5}
      flexShrink={0}
      title={tags.join(", ")}
    >
      {visible.map((tag) => (
        <Box
          key={tag}
          data-testid={`file-tree-tag-dot-${tag}`}
          w="6px"
          h="6px"
          borderRadius="999px"
          bg={pickColor(tag)}
        />
      ))}
      {overflow > 0 && (
        <Box
          data-testid="file-tree-tag-dots-overflow"
          fontSize="9px"
          color="fg.subtle"
          ml={0.5}
          fontFamily="mono"
          lineHeight="1"
        >
          +{overflow}
        </Box>
      )}
    </HStack>
  );
}

/** Stable color pick from `PALETTE` based on the tag string. djb2-ish
 *  hash so similar tags spread across the palette. Exported for tests. */
export function pickColor(tag: string): (typeof PALETTE)[number] {
  let hash = 5381;
  for (let i = 0; i < tag.length; i++) {
    hash = ((hash << 5) + hash + tag.charCodeAt(i)) | 0;
  }
  // Ensure positive index after `| 0` (which can return negative ints).
  const idx = Math.abs(hash) % PALETTE.length;
  return PALETTE[idx]!;
}
