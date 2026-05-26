// Share repo URL dropdown. Reusable trigger +
// popover mounted in BOTH the status bar and the git panel toolbar
// (the "both" decision). All logic lives in `useShareRepoUrl`; this
// is just the Menu.Root chrome around <SharePopover/>.

import { Box, Menu, Portal, chakra } from "@chakra-ui/react";
import { LuShare2 } from "react-icons/lu";

import { SharePopover } from "@/components/layout/share/SharePopover";
import { useShareRepoUrl } from "@/hooks/useShareRepoUrl";

const Trigger = chakra("button");

export interface ShareMenuProps {
  vaultPath: string | null;
  /** "statusbar" renders a compact status-bar cell; "toolbar" a
   *  labelled button for the git panel toolbar. */
  variant?: "statusbar" | "toolbar";
}

export function ShareMenu({
  vaultPath,
  variant = "statusbar",
}: ShareMenuProps) {
  const { options, copy, open } = useShareRepoUrl(vaultPath);

  return (
    <Menu.Root>
      <Menu.Trigger asChild>
        <Trigger
          type="button"
          data-testid="share-menu-trigger"
          data-atom="share-menu-trigger"
          data-variant={variant}
          aria-label="Share repo URL"
          bg="transparent"
          color="fg.muted"
          fontFamily="mono"
          fontSize="11px"
          cursor="pointer"
          display="inline-flex"
          alignItems="center"
          gap={1}
          px={variant === "toolbar" ? 2 : 1}
          flexShrink={0}
          _hover={{ color: "fg" }}
        >
          <LuShare2 size={11} aria-hidden />
          {variant === "toolbar" && <Box as="span">Share</Box>}
        </Trigger>
      </Menu.Trigger>
      <Portal>
        <Menu.Positioner>
          <Menu.Content
            data-testid="share-menu"
            bg="transparent"
            borderWidth={0}
            shadow="none"
            p={0}
          >
            <SharePopover remotes={options} onCopy={copy} onOpen={open} />
          </Menu.Content>
        </Menu.Positioner>
      </Portal>
    </Menu.Root>
  );
}
