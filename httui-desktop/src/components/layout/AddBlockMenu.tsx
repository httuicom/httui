// Floating "+ Add block" menu.
//
// 7 options spanning the executable + non-executable block surface:
// Markdown / HTTP / SQL / MongoDB / WebSocket / GraphQL / Shell.
// Today only HTTP + SQL execute; the remaining four insert a
// placeholder fence with `executable=false` so the document still
// reads as the right block kind.
//
// Pure presentational: takes an `onInsert(template)` callback. The
// parent wires it to a CM6 transaction (insert at cursor, or at end-
// of-doc when the editor isn't focused). Tests + Storybook can pass
// any callback.

import { Box, Menu, Portal, chakra } from "@chakra-ui/react";
import { useCallback } from "react";
import {
  LuPlus,
  LuFileText,
  LuGlobe,
  LuDatabase,
  LuLeaf,
  LuRadio,
  LuShare2,
  LuTerminal,
} from "react-icons/lu";

const Trigger = chakra("button");

export type BlockKind =
  | "markdown"
  | "http"
  | "sql"
  | "mongodb"
  | "websocket"
  | "graphql"
  | "shell";

export interface BlockTemplate {
  kind: BlockKind;
  label: string;
  /** Whether this kind has a real executor today. Non-executable
   * kinds insert with `executable=false` info-string token. */
  executable: boolean;
  /** Template text to insert at cursor. */
  insert: string;
  /** Optional negative offset to land the cursor inside the template
   * (e.g. inside a fence body). */
  cursorOffset?: number;
}

export const BLOCK_TEMPLATES: Readonly<Record<BlockKind, BlockTemplate>> = {
  markdown: {
    kind: "markdown",
    label: "Markdown",
    executable: false,
    insert: "## \n\n",
    cursorOffset: -2,
  },
  http: {
    kind: "http",
    label: "HTTP",
    executable: true,
    insert: "```http alias=req1\nGET \n```\n",
    cursorOffset: -5,
  },
  sql: {
    kind: "sql",
    label: "SQL",
    executable: true,
    insert: "```db alias=q1\nSELECT 1;\n```\n",
    cursorOffset: -5,
  },
  mongodb: {
    kind: "mongodb",
    label: "MongoDB",
    executable: false,
    insert:
      "```mongodb alias=q1 executable=false\ndb.collection.find({})\n```\n",
    cursorOffset: -6,
  },
  websocket: {
    kind: "websocket",
    label: "WebSocket",
    executable: false,
    insert: "```ws alias=ws1 executable=false\nwss://example.com/socket\n```\n",
    cursorOffset: -6,
  },
  graphql: {
    kind: "graphql",
    label: "GraphQL",
    executable: false,
    insert: "```graphql alias=q1 executable=false\nquery {\n  \n}\n```\n",
    cursorOffset: -8,
  },
  shell: {
    kind: "shell",
    label: "Shell",
    executable: false,
    insert: "```sh alias=s1 executable=false\necho hello\n```\n",
    cursorOffset: -6,
  },
};

const ICONS: Record<BlockKind, React.ComponentType<{ size?: number }>> = {
  markdown: LuFileText,
  http: LuGlobe,
  sql: LuDatabase,
  mongodb: LuLeaf,
  websocket: LuRadio,
  graphql: LuShare2,
  shell: LuTerminal,
};

const KIND_ORDER: ReadonlyArray<BlockKind> = [
  "markdown",
  "http",
  "sql",
  "mongodb",
  "websocket",
  "graphql",
  "shell",
];

export interface AddBlockMenuProps {
  onInsert: (template: BlockTemplate) => void;
  /** Optional aria-label on the trigger for screen readers. */
  ariaLabel?: string;
  /** Trigger size (px). Default 32 for the standalone floating
   *  button; pass 20 to fit inside the editor toolbar (28px tall). */
  triggerSize?: number;
}

export function AddBlockMenu({
  onInsert,
  ariaLabel = "Add block",
  triggerSize = 32,
}: AddBlockMenuProps) {
  const handleSelect = useCallback(
    (kind: BlockKind) => {
      onInsert(BLOCK_TEMPLATES[kind]);
    },
    [onInsert],
  );

  const iconSize = Math.round(triggerSize / 2);

  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Trigger
          type="button"
          data-atom="add-block-trigger"
          aria-label={ariaLabel}
          h={`${triggerSize}px`}
          w={`${triggerSize}px`}
          display="inline-flex"
          alignItems="center"
          justifyContent="center"
          borderRadius="full"
          borderWidth="1px"
          borderColor="border"
          bg="bg.muted"
          color="fg.muted"
          cursor="pointer"
          flexShrink={0}
          _hover={{ bg: "bg.emphasized", color: "fg" }}
        >
          <LuPlus size={iconSize} />
        </Trigger>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content
            data-testid="add-block-menu"
            bg="bg"
            borderWidth="1px"
            borderColor="border"
            shadow="2xl"
            minW="180px"
          >
            {KIND_ORDER.map((kind) => {
              const t = BLOCK_TEMPLATES[kind];
              const Icon = ICONS[kind];
              return (
                <Menu.Item
                  key={kind}
                  value={kind}
                  data-block-kind={kind}
                  data-executable={t.executable ? "true" : "false"}
                  onSelect={() => handleSelect(kind)}
                  cursor="pointer"
                  px={2}
                  py={1.5}
                  borderRadius="3px"
                >
                  <Box display="inline-flex" alignItems="center" gap={2}>
                    <Icon size={14} />
                    <Box flex={1}>{t.label}</Box>
                    {!t.executable && (
                      <Box fontSize="10px" color="fg.subtle">
                        non-exec
                      </Box>
                    )}
                  </Box>
                </Menu.Item>
              );
            })}
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
