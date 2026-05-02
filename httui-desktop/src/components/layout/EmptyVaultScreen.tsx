/**
 * Welcome screen rendered by AppShell when no vault is active.
 *
 * Lays out the canvas §3 surface: 260px workspace sidebar + centred
 * main column with three cards — Open / Clone / Create. The cards
 * are dumb; this screen owns the busy/error state and wires each
 * one to the workspace store + Tauri commands.
 *
 * Cenário 1 (V1): Open is fully functional. Clone and Create show
 * forms that surface "not implemented" errors inline. Cenários 2 e 3
 * complete those flows.
 */

import { useCallback, useEffect, useState } from "react";
import { Box, Flex, Heading, Stack, Text } from "@chakra-ui/react";

import { useWorkspaceStore } from "@/stores/workspace";
import { scaffoldVault, writeNote } from "@/lib/tauri/commands";
import { EmptyVaultSidebar } from "@/components/layout/empty-vault/EmptyVaultSidebar";
import { OpenVaultCard } from "@/components/layout/empty-vault/OpenVaultCard";
import { CloneVaultCard } from "@/components/layout/empty-vault/CloneVaultCard";
import { CreateVaultCard } from "@/components/layout/empty-vault/CreateVaultCard";
import { EmptyVaultFooter } from "@/components/layout/empty-vault/EmptyVaultFooter";
import { FUJI_BG_DARK, FUJI_BG_LIGHT } from "@/theme/tokens";
import {
  buildRunbookFromUrl,
  extractUrl,
  PASTE_URL_RUNBOOK_PATH,
} from "@/lib/paste-url";

interface FlowState {
  busy: boolean;
  error: string | null;
}

async function pickDirectory(title: string): Promise<string | null> {
  const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
  const selected = await openDialog({
    directory: true,
    multiple: false,
    title,
  });
  return (selected as string | null) ?? null;
}

export function EmptyVaultScreen() {
  const openVault = useWorkspaceStore((s) => s.openVault);
  const switchVault = useWorkspaceStore((s) => s.switchVault);
  const [flow, setFlow] = useState<FlowState>({ busy: false, error: null });

  const handleOpen = useCallback(async () => {
    setFlow({ busy: true, error: null });
    try {
      await openVault();
    } catch (err) {
      setFlow({
        busy: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return;
    }
    setFlow({ busy: false, error: null });
  }, [openVault]);

  const handleClone = useCallback(
    async (_url: string, _destination: string | null) => {
      throw new Error(
        "Clone ainda não está disponível — chega na próxima vertical (cenário 2).",
      );
    },
    [],
  );

  const handleCreate = useCallback(
    async (_parentPath: string, _name: string) => {
      throw new Error(
        "Create ainda não está disponível — chega na próxima vertical (cenário 3).",
      );
    },
    [],
  );

  const handleSidebarCreate = useCallback(async () => {
    setFlow({ busy: true, error: null });
    try {
      const picked = await pickDirectory("Choose folder for new vault");
      if (!picked) {
        setFlow({ busy: false, error: null });
        return;
      }
      await scaffoldVault(picked);
      await switchVault(picked);
    } catch (err) {
      setFlow({
        busy: false,
        error: err instanceof Error ? err.message : String(err),
      });
      return;
    }
    setFlow({ busy: false, error: null });
  }, [switchVault]);

  // Paste-URL flow (Epic 41 Story 06): when the user pastes a clean
  // http(s) URL while on the empty-vault screen, scaffold a vault and
  // seed it with `runbooks/untitled.md` containing a runnable HTTP
  // GET block for that URL. Non-URL pastes fall through to the OS
  // default. Listens at document level so the user doesn't have to
  // click anything first.
  const handleCreateWithUrl = useCallback(
    async (url: string) => {
      setFlow({ busy: true, error: null });
      try {
        const picked = await pickDirectory("Choose folder for new vault");
        if (!picked) {
          setFlow({ busy: false, error: null });
          return;
        }
        await scaffoldVault(picked);
        await writeNote(picked, PASTE_URL_RUNBOOK_PATH, buildRunbookFromUrl(url));
        await switchVault(picked);
      } catch (err) {
        setFlow({
          busy: false,
          error: err instanceof Error ? err.message : String(err),
        });
        return;
      }
      setFlow({ busy: false, error: null });
    },
    [switchVault],
  );

  useEffect(() => {
    function onPaste(event: ClipboardEvent) {
      const text = event.clipboardData?.getData("text/plain") ?? "";
      const url = extractUrl(text);
      if (!url) return;
      const target = event.target;
      if (target instanceof Element) {
        if (target.matches("input, textarea, [contenteditable]")) {
          return;
        }
      }
      event.preventDefault();
      void handleCreateWithUrl(url);
    }
    document.addEventListener("paste", onPaste);
    return () => {
      document.removeEventListener("paste", onPaste);
    };
  }, [handleCreateWithUrl]);

  const pickClonePath = useCallback(
    () => pickDirectory("Choose destination for cloned vault"),
    [],
  );
  const pickCreateParent = useCallback(
    () => pickDirectory("Choose parent folder for new vault"),
    [],
  );

  return (
    <Flex
      data-testid="empty-vault-screen"
      data-fuji-bg="true"
      flex={1}
      bg="bg.subtle"
      backgroundImage={{
        _light: FUJI_BG_LIGHT,
        _dark: FUJI_BG_DARK,
      }}
      backgroundSize="cover"
      backgroundRepeat="no-repeat"
    >
      <EmptyVaultSidebar onCreateRunbook={handleSidebarCreate} />
      <Flex flex={1} align="center" justify="center" px={8} py={12}>
        <Stack maxW="900px" gap={6} align="stretch">
          <Box>
            <Text
              fontSize="xs"
              fontWeight="bold"
              letterSpacing="0.12em"
              textTransform="uppercase"
              color="brand.500"
            >
              Workspace ready
            </Text>
            <Heading as="h1" size="2xl" mt={2}>
              Welcome to httui notes
            </Heading>
            <Text mt={3} fontSize="md" color="fg.muted">
              Each runbook is a `.md` file you read, run, and version. Open an
              existing folder, clone a teammate&apos;s repo, or start fresh.
            </Text>
          </Box>

          {flow.error && (
            <Box
              data-testid="empty-vault-error"
              bg="red.50"
              color="red.900"
              border="1px solid"
              borderColor="red.200"
              borderRadius="md"
              px={4}
              py={3}
            >
              <Text fontSize="sm" fontWeight="bold">
                Couldn&apos;t open the vault
              </Text>
              <Text fontSize="sm" mt={1}>
                {flow.error}
              </Text>
            </Box>
          )}

          <Box
            data-testid="empty-vault-card-grid"
            display="grid"
            gridTemplateColumns={{ base: "1fr", md: "1fr 1fr 1fr" }}
            gap="14px"
            maxW="900px"
            alignItems="stretch"
          >
            <OpenVaultCard onOpenClick={handleOpen} busy={flow.busy} />
            <CloneVaultCard
              onClone={handleClone}
              onPickDestination={pickClonePath}
              busy={flow.busy}
            />
            <CreateVaultCard
              onCreate={handleCreate}
              onPickParent={pickCreateParent}
              busy={flow.busy}
            />
          </Box>

          <EmptyVaultFooter />
        </Stack>
      </Flex>
    </Flex>
  );
}
