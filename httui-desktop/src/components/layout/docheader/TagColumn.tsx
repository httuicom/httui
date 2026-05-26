// Tag column for the DocHeader card.
//
// Pure presentational. Consumer feeds the file's current tags +
// the vault-wide tag union (carries — supplied via store)
// and wires the add/remove/select callbacks.
//
// Each tag renders as a chip with an inline × remove button. The
// "+ Add tag" toggle reveals an Input with autocomplete suggestions
// drawn from `availableTags` minus the currently-applied set.

import { useMemo, useState } from "react";
import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";

export interface TagColumnProps {
  tags: ReadonlyArray<string>;
  /** Union of tags across the vault — feeds the autocomplete. The
   * consumer (page mount) reads this from the tag-index
   *  store; until that store ships, the consumer can pass an
   *  empty array and autocomplete simply doesn't surface. */
  availableTags?: ReadonlyArray<string>;
  /** True while a tag mutation is in flight. Disables every
   *  interactive element; consumer-driven. */
  busy?: boolean;
  onSelectTag?: (tag: string) => void;
  onAddTag?: (tag: string) => void;
  onRemoveTag?: (tag: string) => void;
}

export function TagColumn({
  tags,
  availableTags,
  busy,
  onSelectTag,
  onAddTag,
  onRemoveTag,
}: TagColumnProps) {
  const [adding, setAdding] = useState(false);
  const [input, setInput] = useState("");

  const trimmed = input.trim();
  const tagsSet = useMemo(() => new Set(tags), [tags]);
  const suggestions = useMemo(() => {
    if (!availableTags || trimmed.length === 0) return [];
    const q = trimmed.toLowerCase();
    return availableTags
      .filter((t) => !tagsSet.has(t) && t.toLowerCase().includes(q))
      .slice(0, 6);
  }, [availableTags, trimmed, tagsSet]);

  const submit = (value: string) => {
    const v = value.trim();
    if (v.length === 0 || tagsSet.has(v)) return;
    onAddTag?.(v);
    setInput("");
    setAdding(false);
  };

  return (
    <Box
      data-testid="tag-column"
      data-busy={busy || undefined}
      data-adding={adding || undefined}
      data-tag-count={tags.length}
    >
      <Flex direction="column" gap={1} align="flex-end">
        <Text
          fontFamily="mono"
          fontSize="10px"
          color="fg.subtle"
          textTransform="uppercase"
          letterSpacing="0.05em"
          mb={1}
        >
          Tags
        </Text>
        {tags.length === 0 && !adding && !onAddTag && (
          <Text
            data-testid="tag-column-empty"
            fontFamily="mono"
            fontSize="10px"
            color="fg.subtle"
          >
            No tags
          </Text>
        )}
        {tags.map((tag) => (
          <TagChip
            key={tag}
            tag={tag}
            busy={busy}
            onSelect={onSelectTag}
            onRemove={onRemoveTag}
          />
        ))}
        {adding ? (
          <Box data-testid="tag-column-add-form" w="160px">
            <Input
              data-testid="tag-column-add-input"
              autoFocus
              placeholder="tag"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              disabled={busy}
              onKeyDown={(e) => {
                if (e.key === "Enter") submit(input);
                else if (e.key === "Escape") {
                  setInput("");
                  setAdding(false);
                }
              }}
            />
            {suggestions.length > 0 && (
              <Flex
                data-testid="tag-column-suggestions"
                direction="column"
                gap={0}
                mt={1}
                bg="bg.subtle"
                borderWidth="1px"
                borderColor="border"
                borderRadius="4px"
              >
                {suggestions.map((s) => (
                  <chakra.button
                    key={s}
                    data-testid={`tag-column-suggestion-${s}`}
                    fontFamily="mono"
                    fontSize="11px"
                    color="fg.muted"
                    px={2}
                    py={1}
                    textAlign="left"
                    onClick={() => submit(s)}
                    disabled={busy}
                    _hover={{ bg: "bg.muted" }}
                    cursor="pointer"
                  >
                    {s}
                  </chakra.button>
                ))}
              </Flex>
            )}
          </Box>
        ) : (
          onAddTag && (
            <Btn
              data-testid="tag-column-add"
              variant="ghost"
              onClick={() => setAdding(true)}
              disabled={busy}
            >
              + Add tag
            </Btn>
          )
        )}
      </Flex>
    </Box>
  );
}

function TagChip({
  tag,
  busy,
  onSelect,
  onRemove,
}: {
  tag: string;
  busy?: boolean;
  onSelect?: (tag: string) => void;
  onRemove?: (tag: string) => void;
}) {
  const interactive = !!onSelect && !busy;
  return (
    <Flex
      data-testid={`tag-column-chip-${tag}`}
      align="center"
      gap={1}
      role="group"
    >
      <Text
        as={interactive ? "button" : "span"}
        data-testid={`tag-column-chip-${tag}-label`}
        fontFamily="mono"
        fontSize="11px"
        color="fg.muted"
        cursor={interactive ? "pointer" : undefined}
        onClick={interactive ? () => onSelect(tag) : undefined}
      >
        #{tag}
      </Text>
      {onRemove && (
        <chakra.button
          data-testid={`tag-column-chip-${tag}-remove`}
          fontFamily="mono"
          fontSize="11px"
          color="fg.subtle"
          onClick={() => onRemove(tag)}
          disabled={busy}
          cursor="pointer"
          opacity={0}
          transition="opacity 80ms ease-out"
          _groupHover={{ opacity: 1 }}
          _hover={{ color: "error" }}
        >
          ×
        </chakra.button>
      )}
    </Flex>
  );
}
