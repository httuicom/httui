import { Box, Menu, Portal, chakra } from "@chakra-ui/react";
import { LuCheck, LuChevronDown, LuFolderOpen } from "react-icons/lu";

const Trigger = chakra("button");

export interface WorkspaceMenuProps {
  workspace: string;
  /** Whether this is the deepest breadcrumb segment (drives fg vs fg.muted). */
  isLeaf: boolean;
  vaults: string[];
  activeVault: string | null;
  onSwitch: (path: string) => void;
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
                  _highlighted={{ bg: "brand.subtle", color: "fg" }}
                  _hover={{ bg: "brand.subtle", color: "fg" }}
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
              _highlighted={{ bg: "bg.muted", color: "fg" }}
              _hover={{ bg: "bg.muted", color: "fg" }}
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
