// Workspace dropdown — replaces the previous "click to cycle" behavior
// on the breadcrumb workspace segment. Lists every known vault with
// the active one marked, plus an "Abrir outro vault…" item that opens
// a directory picker.
//
// Pure presentational over the workspace store: parent wires
// `vaults`, `activeVault`, `onSwitch`, `onOpenOther`. Style mirrors
// the breadcrumb workspace Segment (transparent bg, fg.2 default,
// fg on hover, ellipsis at 160px) so the trigger feels at home in
// the breadcrumb row.

import { Box, Menu, Portal, chakra } from "@chakra-ui/react";
import { LuCheck, LuChevronDown, LuFolderOpen } from "react-icons/lu";

const Trigger = chakra("button");

export interface WorkspaceMenuProps {
  /** Active vault's basename (display label). */
  workspace: string;
  /** Whether this segment is the deepest one in the breadcrumb (no
   * file segments to the right). Drives the `fg` vs `fg.2` color so
   * the trigger matches the rest of the breadcrumb. */
  isLeaf: boolean;
  /** All known vaults (absolute paths). Empty array means only the
   * "Open other vault…" item is shown. */
  vaults: string[];
  /** Active vault's absolute path — used to mark the current item. */
  activeVault: string | null;
  /** Switch to the picked vault. */
  onSwitch: (path: string) => void;
  /** Trigger the directory picker for a brand-new vault. */
  onOpenOther: () => void;
}

function basename(path: string): string {
  return path.split("/").filter(Boolean).pop() ?? path;
}

export function WorkspaceMenu({
  workspace,
  isLeaf,
  vaults,
  activeVault,
  onSwitch,
  onOpenOther,
}: WorkspaceMenuProps) {
  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Trigger
          type="button"
          data-atom="workspace-trigger"
          data-segment="workspace"
          aria-label={`Workspace ${workspace}`}
          bg="transparent"
          color={isLeaf ? "fg" : "fg.muted"}
          fontWeight={500}
          fontSize="13px"
          cursor="pointer"
          display="inline-flex"
          alignItems="center"
          gap={1}
          px={1}
          maxWidth="160px"
          overflow="hidden"
          flexShrink={0}
          _hover={{ color: "fg" }}
          title={workspace}
        >
          <Box
            as="span"
            overflow="hidden"
            textOverflow="ellipsis"
            whiteSpace="nowrap"
          >
            {workspace}
          </Box>
          <LuChevronDown size={11} aria-hidden />
        </Trigger>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content
            data-testid="workspace-menu"
            minW="240px"
            maxW="360px"
            borderWidth="1px"
            borderColor="border"
            shadow="2xl"
          >
            {vaults.map((vault) => {
              const isActive = vault === activeVault;
              return (
                <Menu.Item
                  key={vault}
                  value={vault}
                  data-vault-path={vault}
                  data-active={isActive ? "true" : "false"}
                  onSelect={() => onSwitch(vault)}
                  cursor="pointer"
                  px={2}
                  py={1.5}
                  borderRadius="3px"
                  _highlighted={{ bg: "accent.soft", color: "fg" }}
                  _hover={{ bg: "accent.soft", color: "fg" }}
                >
                  <Box
                    display="inline-flex"
                    alignItems="center"
                    gap={2}
                    w="100%"
                  >
                    <Box w="14px" display="inline-flex" justifyContent="center">
                      {isActive && <LuCheck size={12} />}
                    </Box>
                    <Box flex={1} minW={0}>
                      <Box
                        fontSize="13px"
                        fontWeight={isActive ? 600 : 500}
                        overflow="hidden"
                        textOverflow="ellipsis"
                        whiteSpace="nowrap"
                      >
                        {basename(vault)}
                      </Box>
                      <Box
                        fontSize="10px"
                        color="fg.subtle"
                        overflow="hidden"
                        textOverflow="ellipsis"
                        whiteSpace="nowrap"
                      >
                        {vault}
                      </Box>
                    </Box>
                  </Box>
                </Menu.Item>
              );
            })}
            {vaults.length > 0 && <Menu.Separator />}
            <Menu.Item
              value="open-other"
              data-testid="workspace-open-other"
              onSelect={onOpenOther}
              cursor="pointer"
              px={2}
              py={1.5}
              borderRadius="3px"
              _highlighted={{ bg: "sel", color: "fg" }}
              _hover={{ bg: "sel", color: "fg" }}
            >
              <Box display="inline-flex" alignItems="center" gap={2}>
                <Box w="14px" display="inline-flex" justifyContent="center">
                  <LuFolderOpen size={12} />
                </Box>
                <Box>Abrir outro vault…</Box>
              </Box>
            </Menu.Item>
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
