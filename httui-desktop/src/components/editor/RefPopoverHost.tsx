// Mounts the inline `{{ref}}` popover over CM6 (V11 cenário 3 + 6).
//
// Uses Chakra's Popover (Ark) with a virtual anchor — same pattern as
// EnvironmentsPage / the EnvSwitcher clone popover. Chakra owns
// positioning, Esc and interact-outside; `autoFocus={false}` keeps
// it from trapping focus, and our `onOpenChange → closeRefPopover()`
// restores the caret + editor focus (no Popover.Trigger exists, so
// Ark has nothing to yank focus back to). Not a Dialog → CM6 stays
// keyboard-driveable.

import { Popover, Portal } from "@chakra-ui/react";
import { useCallback, useSyncExternalStore } from "react";

import { useWorkspaceStore } from "@/stores/workspace";
import {
  closeRefPopover,
  getRefPopoverState,
  subscribeRefPopover,
} from "@/lib/blocks/cm-ref-popover";

import { RefPopover } from "./RefPopover";

export function RefPopoverHost() {
  const state = useSyncExternalStore(
    subscribeRefPopover,
    getRefPopoverState,
    getRefPopoverState,
  );
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);

  // Virtual anchor — resolved every positioning tick from the chip
  // rect captured when the popover opened.
  const getAnchorRect = useCallback(() => {
    const s = getRefPopoverState();
    if (!s) return null;
    const { left, top, right, bottom } = s.rect;
    return { x: left, y: top, width: right - left, height: bottom - top };
  }, []);

  return (
    <Popover.Root
      open={!!state}
      onOpenChange={(e) => {
        if (!e.open) closeRefPopover();
      }}
      autoFocus={false}
      lazyMount
      unmountOnExit
      positioning={{
        placement: "bottom-start",
        getAnchorRect,
        gutter: 6,
      }}
    >
      <Portal>
        <Popover.Positioner>
          <Popover.Content
            data-testid="ref-popover-host"
            width="auto"
            bg="transparent"
            borderWidth={0}
            boxShadow="none"
            p={0}
          >
            {state && (
              <RefPopover
                state={state}
                vaultPath={vaultPath}
                onClose={() => closeRefPopover()}
              />
            )}
          </Popover.Content>
        </Popover.Positioner>
      </Portal>
    </Popover.Root>
  );
}
