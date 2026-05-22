import { useEffect, useRef, useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";
import { MergeView } from "@codemirror/merge";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { Btn } from "@/components/atoms";
import type { ConflictVersions } from "@/lib/tauri/git";

const readOnlyExtension = EditorState.readOnly.of(true);
const themeExtension = EditorView.theme({
  "&": { height: "100%", fontSize: "13px" },
  ".cm-content": {
    fontFamily: "var(--chakra-fonts-mono)",
    padding: "8px 0",
  },
  ".cm-gutters": { background: "transparent", borderRight: "none" },
  ".cm-line": { padding: "0 8px" },
});

export interface GitConflictResolverProps {
  path: string;
  versions: ConflictVersions;
  busy?: boolean;
  /** Fires with the final merged text (left pane) when the user
   *  marks the file resolved. */
  onResolve: (path: string, merged: string) => void;
  onCancel: () => void;
}

export function GitConflictResolver({
  path,
  versions,
  busy,
  onResolve,
  onCancel,
}: GitConflictResolverProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const mergeViewRef = useRef<MergeView | null>(null);
  const [showBase, setShowBase] = useState(false);

  useEffect(() => {
    if (!containerRef.current) return;
    const view = new MergeView({
      a: {
        doc: versions.ours,
        extensions: [themeExtension],
      },
      b: {
        doc: versions.theirs,
        extensions: [readOnlyExtension, themeExtension],
      },
      parent: containerRef.current,
      highlightChanges: true,
      gutter: true,
      collapseUnchanged: { margin: 3, minSize: 6 },
    });
    mergeViewRef.current = view;
    return () => {
      view.destroy();
      mergeViewRef.current = null;
    };
  }, [versions]);

  const markResolved = () => {
    const merged = mergeViewRef.current?.a.state.doc.toString();
    if (merged === undefined) return;
    onResolve(path, merged);
  };

  return (
    <Flex
      data-testid="git-conflict-resolver"
      data-path={path}
      direction="column"
      h="100%"
      minH={0}
    >
      <Flex
        align="center"
        gap={2}
        px={3}
        py={2}
        borderBottomWidth="1px"
        borderBottomColor="border"
        bg="bg.subtle"
        flexShrink={0}
      >
        <Text fontFamily="mono" fontSize="11px" color="fg" flex={1} truncate>
          Resolving {path} — ours (editable) ↔ theirs
        </Text>
        <Btn
          data-testid="git-conflict-resolver-base-toggle"
          variant="ghost"
          onClick={() => setShowBase((v) => !v)}
        >
          {showBase ? "Hide base" : "View base"}
        </Btn>
      </Flex>

      {showBase && (
        <Box
          data-testid="git-conflict-resolver-base"
          maxH="25%"
          overflow="auto"
          borderBottomWidth="1px"
          borderBottomColor="border"
          bg="bg.muted"
          px={3}
          py={2}
        >
          <Text
            as="div"
            fontFamily="mono"
            fontSize="10px"
            textTransform="uppercase"
            color="fg.subtle"
            mb={1}
          >
            Merge base
          </Text>
          <Box
            as="pre"
            m={0}
            fontFamily="mono"
            fontSize="11px"
            color="fg.muted"
          >
            {versions.base.length > 0
              ? versions.base
              : "(no common ancestor — add/add conflict)"}
          </Box>
        </Box>
      )}

      <Box ref={containerRef} flex="1 1 auto" minH={0} overflow="auto" />

      <Flex
        gap={2}
        px={3}
        py={2}
        borderTopWidth="1px"
        borderTopColor="border"
        bg="bg.subtle"
        flexShrink={0}
      >
        <Btn
          data-testid="git-conflict-resolver-mark"
          variant="primary"
          disabled={busy}
          onClick={markResolved}
        >
          Mark resolved
        </Btn>
        <Btn
          data-testid="git-conflict-resolver-cancel"
          variant="ghost"
          disabled={busy}
          onClick={onCancel}
        >
          Cancel
        </Btn>
      </Flex>
    </Flex>
  );
}
