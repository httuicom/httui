// V2 / cenário 4.5 / M6 — editable pre-flight checklist for the
// DocHeader. Displayed below the tag column when the doc has any
// items, or when the consumer is in editable mode (`onChecklistSave`
// provided).
//
// Storage shape lives in `frontmatter.preflight` as a list of
// `{ text, done }` items. The consumer rebuilds the full list on each
// edit (toggle / text change / add / remove) and calls
// `onChecklistSave` with the new array; the writer
// (`updateFrontmatterPreflight`) handles the YAML round-trip.
//
// Static rendering (no `onChecklistSave`) shows the same rows as
// disabled boxes so the diff viewer / read-only contexts still display
// the checklist visually.

import { useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";
import type { PreflightItem } from "@/lib/blocks/preflight-item";

export interface DocHeaderChecklistProps {
  items: ReadonlyArray<PreflightItem>;
  /** When provided, the checklist becomes editable (toggle done /
   *  edit text / add / remove). The consumer receives the full new
   *  list on every change. */
  onChecklistSave?: (items: PreflightItem[]) => void;
}

export function DocHeaderChecklist({
  items,
  onChecklistSave,
}: DocHeaderChecklistProps) {
  const editable = onChecklistSave !== undefined;
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState("");

  if (!editable && items.length === 0) return null;

  const commit = (next: PreflightItem[]) => {
    onChecklistSave?.(next);
  };

  const toggle = (idx: number) => {
    const next = items.map((it, i) =>
      i === idx ? { ...it, done: !it.done } : it,
    );
    commit(next);
  };

  const remove = (idx: number) => {
    commit(items.filter((_, i) => i !== idx));
  };

  const updateText = (idx: number, text: string) => {
    const trimmed = text.trim();
    if (trimmed.length === 0) {
      remove(idx);
      return;
    }
    const next = items.map((it, i) =>
      i === idx ? { ...it, text: trimmed } : it,
    );
    commit(next);
  };

  const addDraft = (value: string) => {
    const trimmed = value.trim();
    if (trimmed.length === 0) {
      setAdding(false);
      setDraft("");
      return;
    }
    commit([...items, { text: trimmed, done: false }]);
    setDraft("");
    setAdding(false);
  };

  return (
    <Box data-testid="docheader-checklist" mt={3}>
      <Flex direction="column" gap={1}>
        {items.map((item, idx) => (
          <ChecklistRow
            key={idx}
            item={item}
            editable={editable}
            onToggle={() => toggle(idx)}
            onCommitText={(text) => updateText(idx, text)}
            onRemove={() => remove(idx)}
          />
        ))}
        {editable &&
          (adding ? (
            <Box data-testid="docheader-checklist-add-form" w="240px">
              <Input
                data-testid="docheader-checklist-add-input"
                autoFocus
                placeholder="Add a check…"
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addDraft(draft);
                  } else if (e.key === "Escape") {
                    setDraft("");
                    setAdding(false);
                  }
                }}
                onBlur={() => addDraft(draft)}
              />
            </Box>
          ) : (
            <Btn
              data-testid="docheader-checklist-add"
              variant="ghost"
              onClick={() => setAdding(true)}
            >
              + Add check
            </Btn>
          ))}
      </Flex>
    </Box>
  );
}

interface ChecklistRowProps {
  item: PreflightItem;
  editable: boolean;
  onToggle: () => void;
  onCommitText: (text: string) => void;
  onRemove: () => void;
}

function ChecklistRow({
  item,
  editable,
  onToggle,
  onCommitText,
  onRemove,
}: ChecklistRowProps) {
  const [editingText, setEditingText] = useState(false);
  const [draft, setDraft] = useState(item.text);
  return (
    <Flex
      data-testid="docheader-checklist-row"
      data-done={item.done || undefined}
      align="center"
      gap={2}
    >
      <Box
        as="button"
        type="button"
        data-testid="docheader-checklist-checkbox"
        data-checked={item.done || undefined}
        onClick={editable ? onToggle : undefined}
        disabled={!editable}
        w="14px"
        h="14px"
        borderWidth="1px"
        borderColor={item.done ? "accent" : "line"}
        borderRadius="3px"
        bg={item.done ? "accent" : "transparent"}
        cursor={editable ? "pointer" : "default"}
        flexShrink={0}
        position="relative"
        _after={
          item.done
            ? {
                content: '"✓"',
                position: "absolute",
                top: "-4px",
                left: "1px",
                color: "bg",
                fontSize: "11px",
                fontWeight: 700,
              }
            : undefined
        }
      />
      {editable && editingText ? (
        <Box
          as="input"
          data-testid="docheader-checklist-text-input"
          type="text"
          value={draft}
          autoFocus
          onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
            setDraft(e.target.value)
          }
          onBlur={() => {
            setEditingText(false);
            if (draft !== item.text) onCommitText(draft);
          }}
          onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => {
            if (e.key === "Enter") {
              e.preventDefault();
              setEditingText(false);
              if (draft !== item.text) onCommitText(draft);
            } else if (e.key === "Escape") {
              setDraft(item.text);
              setEditingText(false);
            }
          }}
          fontFamily="mono"
          fontSize="12px"
          color={item.done ? "fg.3" : "fg"}
          bg="transparent"
          border="none"
          outline="none"
          flex={1}
          m={0}
          p={0}
        />
      ) : (
        <Text
          as={editable ? "button" : "span"}
          data-testid="docheader-checklist-text"
          fontFamily="mono"
          fontSize="12px"
          color={item.done ? "fg.3" : "fg"}
          textDecoration={item.done ? "line-through" : undefined}
          textAlign="left"
          flex={1}
          onClick={
            editable
              ? () => {
                  setDraft(item.text);
                  setEditingText(true);
                }
              : undefined
          }
          cursor={editable ? "text" : undefined}
        >
          {item.text}
        </Text>
      )}
      {editable && (
        <Text
          as="button"
          type="button"
          data-testid="docheader-checklist-remove"
          fontFamily="mono"
          fontSize="11px"
          color="fg.3"
          onClick={onRemove}
          cursor="pointer"
          _hover={{ color: "error" }}
        >
          ×
        </Text>
      )}
    </Flex>
  );
}
