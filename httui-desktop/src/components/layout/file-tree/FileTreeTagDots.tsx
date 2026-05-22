import { Box, HStack } from "@chakra-ui/react";

import { useTagIndexStore } from "@/stores/tagIndex";

const MAX_DOTS = 3;

// Six muted swatches, Chakra semantic tokens so theme switches stay live.
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

/** djb2-ish hash → stable palette index. Exported for tests. */
export function pickColor(tag: string): (typeof PALETTE)[number] {
  let hash = 5381;
  for (let i = 0; i < tag.length; i++) {
    hash = ((hash << 5) + hash + tag.charCodeAt(i)) | 0;
  }
  // `| 0` can return negative ints; abs ensures a valid index.
  const idx = Math.abs(hash) % PALETTE.length;
  return PALETTE[idx]!;
}
