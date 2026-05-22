import { useState } from "react";
import { Box, Flex, Text, chakra } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";
import type { TaskItem } from "@/lib/blocks/task-item";

export interface DocHeaderChecklistProps {
  items: ReadonlyArray<TaskItem>;
  /** When provided, the checklist becomes editable (toggle done /
   *  edit text / add / remove). The consumer receives the full new
   *  list on every change. */
  onChecklistSave?: (items: TaskItem[]) => void;
}

export function DocHeaderChecklist({
  items,
  onChecklistSave,
}: DocHeaderChecklistProps) {
  const editable = onChecklistSave !== undefined;
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState("");

  if (!editable && items.length === 0) return null;

  const commit = (next: TaskItem[]) => {
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

  const n = items.length;
  const headerLabel =
    n === 0 ? "Pré-flight" : `Pré-flight — ${n} ${n === 1 ? "item" : "itens"}`;

  return (
    <Box
      data-testid="docheader-checklist"
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      bg="bg.subtle"
      px={4}
      py={3}
    >
      <Text
        fontFamily="mono"
        fontSize="10px"
        color="fg.subtle"
        textTransform="uppercase"
        letterSpacing="0.05em"
        mb={2}
      >
        {headerLabel}
      </Text>
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
            <Box>
              <Btn
                data-testid="docheader-checklist-add"
                variant="ghost"
                onClick={() => setAdding(true)}
              >
                + Add check
              </Btn>
            </Box>
          ))}
      </Flex>
    </Box>
  );
}

interface ChecklistRowProps {
  item: TaskItem;
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
      <chakra.button
        type="button"
        data-testid="docheader-checklist-checkbox"
        data-checked={item.done ? "true" : undefined}
        onClick={editable ? onToggle : undefined}
        disabled={!editable}
        w="16px"
        h="16px"
        display="flex"
        alignItems="center"
        justifyContent="center"
        borderWidth="1px"
        borderRadius="3px"
        cursor={editable ? "pointer" : "default"}
        flexShrink={0}
        p={0}
        css={{
          // User-agent stylesheets repaint <button> backgrounds via
          // `background-color: buttonface` (and similar) which beats
          // Chakra's class-scoped rules in the cascade. `appearance:
          // none` clears that, then we read the semantic-token CSS
          // vars directly. The vars are emitted by Chakra v3 from
          // `lib/theme.ts`'s semanticTokens config (see index.css for
          // the same `--chakra-colors-bg-3` / `--chakra-colors-fg-3`
          // pattern in production).
          appearance: "none",
          backgroundColor: "transparent",
          borderColor: "var(--chakra-colors-line)",
          "&[data-checked='true']": {
            backgroundColor: "var(--chakra-colors-accent)",
            borderColor: "var(--chakra-colors-accent)",
          },
        }}
      >
        {item.done && (
          <Text
            as="span"
            fontSize="11px"
            lineHeight="1"
            color="var(--chakra-colors-accent-fg)"
            fontWeight={700}
            aria-hidden="true"
          >
            ✓
          </Text>
        )}
      </chakra.button>
      {editable && editingText ? (
        <chakra.input
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
          color={item.done ? "fg.subtle" : "fg"}
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
          color={item.done ? "fg.subtle" : "fg"}
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
        <chakra.button
          type="button"
          data-testid="docheader-checklist-remove"
          fontFamily="mono"
          fontSize="11px"
          color="fg.subtle"
          onClick={onRemove}
          cursor="pointer"
          _hover={{ color: "error" }}
        >
          ×
        </chakra.button>
      )}
    </Flex>
  );
}
