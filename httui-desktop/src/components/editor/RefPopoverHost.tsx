// Mounts the inline `{{ref}}` popover over CM6 (V11 cenário 3 + 6).
//
// Subscribes to the cm-ref-popover emitter; when a chip is clicked it
// renders <RefPopover> in a Portal+Box (no Dialog → CM6 stays
// focusable) anchored under the chip. Esc / outside-click close and
// restore the caret + editor focus.

import { Box, Portal } from "@chakra-ui/react";
import { useEffect, useRef, useSyncExternalStore } from "react";

import { useEscapeClose } from "@/hooks/useEscapeClose";
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
  const boxRef = useRef<HTMLDivElement | null>(null);

  useEscapeClose(() => {
    if (getRefPopoverState()) closeRefPopover();
  });

  useEffect(() => {
    if (!state) return;
    // Defer one tick so the mousedown that opened the popover doesn't
    // immediately count as an outside click.
    let armed = false;
    const arm = setTimeout(() => {
      armed = true;
    }, 0);
    const onDown = (e: MouseEvent) => {
      if (!armed) return;
      if (boxRef.current && !boxRef.current.contains(e.target as Node)) {
        closeRefPopover();
      }
    };
    document.addEventListener("mousedown", onDown, true);
    return () => {
      clearTimeout(arm);
      document.removeEventListener("mousedown", onDown, true);
    };
  }, [state]);

  if (!state) return null;

  // Anchor under the chip, clamped into the viewport.
  const margin = 8;
  const width = 420;
  const left = Math.max(
    margin,
    Math.min(state.rect.left, window.innerWidth - width - margin),
  );
  const top = state.rect.bottom + 4;

  return (
    <Portal>
      <Box
        ref={boxRef}
        data-testid="ref-popover-host"
        position="fixed"
        left={`${left}px`}
        top={`${top}px`}
        zIndex={1400}
      >
        <RefPopover
          state={state}
          vaultPath={vaultPath}
          onClose={() => closeRefPopover()}
        />
      </Box>
    </Portal>
  );
}
