// Smart wrapper around <EnvMenu /> (V11 cenário 1). Owns:
// - env store wiring (list / active / switch / duplicate)
// - the ⌘E open flag (useEnvSwitcherStore — opened from AppShell)
// - the "Clone <active>" footer action → reuses the V5 carry
//   CloneEnvironmentForm in a Portal+Popover anchored to the
//   status-bar env cell (Popover, not Dialog — preserves CM6 focus).
//
// StatusBar renders this instead of <EnvMenu /> directly.

import { Box, Popover, Portal } from "@chakra-ui/react";
import { useCallback, useState } from "react";

import {
  CloneEnvironmentForm,
  type CloneEnvironmentPayload,
} from "@/components/layout/environments/CloneEnvironmentForm";
import { useEnvironmentStore } from "@/stores/environment";
import { useEnvSwitcherStore } from "@/stores/envSwitcher";

import { EnvMenu } from "./EnvMenu";

export function EnvSwitcher() {
  const environments = useEnvironmentStore((s) => s.environments);
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
  const switchEnvironment = useEnvironmentStore((s) => s.switchEnvironment);
  const duplicateEnvironment = useEnvironmentStore(
    (s) => s.duplicateEnvironment,
  );
  const open = useEnvSwitcherStore((s) => s.open);
  const setOpen = useEnvSwitcherStore((s) => s.setOpen);

  const [cloning, setCloning] = useState(false);

  const getAnchorRect = useCallback(() => {
    const cell = document.querySelector<HTMLElement>(
      '[data-testid="status-env"]',
    );
    return cell?.getBoundingClientRect() ?? null;
  }, []);

  const handleRequestClone = useCallback(() => {
    setOpen(false);
    setCloning(true);
  }, [setOpen]);

  const handleCloneSubmit = useCallback(
    async (payload: CloneEnvironmentPayload) => {
      if (!activeEnvironment) return;
      await duplicateEnvironment(activeEnvironment.id, payload.name);
      setCloning(false);
    },
    [activeEnvironment, duplicateEnvironment],
  );

  return (
    <>
      <EnvMenu
        environments={environments}
        activeEnvironment={activeEnvironment}
        onSwitch={(id) => void switchEnvironment(id)}
        open={open}
        onOpenChange={setOpen}
        onRequestClone={handleRequestClone}
      />

      <Popover.Root
        open={cloning && !!activeEnvironment}
        onOpenChange={(e) => {
          if (!e.open) setCloning(false);
        }}
        positioning={{
          placement: "bottom-start",
          getAnchorRect,
          gutter: 8,
        }}
      >
        <Portal>
          <Popover.Positioner>
            <Box
              data-testid="env-switcher-clone"
              minW="360px"
              maxW="480px"
              filter="drop-shadow(0 8px 24px rgba(0,0,0,0.15))"
            >
              {cloning && activeEnvironment && (
                <CloneEnvironmentForm
                  sourceFilename={`${activeEnvironment.name}.toml`}
                  sourceName={activeEnvironment.name}
                  existingFilenames={environments.map((e) => `${e.name}.toml`)}
                  onSubmit={handleCloneSubmit}
                  onCancel={() => setCloning(false)}
                />
              )}
            </Box>
          </Popover.Positioner>
        </Portal>
      </Popover.Root>
    </>
  );
}
