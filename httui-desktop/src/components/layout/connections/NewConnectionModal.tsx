// Canvas §5 — "Nova conexão" modal shell (Epic 42 Story 06 — Phase 1).
//
// Outer 880×~660 modal centered on a dimmed page bg. Two-column
// grid: 220px sidebar pick-kind + 1fr form area. Form area drives 4
// tabs (Form / Connection string / SSH tunnel / SSL); a `renderTabBody`
// prop lets phase 2+3 inject the per-tab panels without forcing this
// shell to grow. Phase 1 ships the layout + dispatch surface only.
//
// Tab strip uses the design-system `Tabbar` atom — its active state
// renders a 1px top accent line (canvas §0). The Story 06 prose says
// "2px accent underline"; we follow the atom to keep the design
// system the single source of truth (audit-034).
//
// Pure presentational — owns selectedKind + activeTab so consumers
// don't need to thread the wiring; save/test/cancel callbacks are
// dispatched up.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { Box, Flex, Portal, Text } from "@chakra-ui/react";
import { LuPlay } from "react-icons/lu";

import { Btn, Tabbar, type TabItem } from "@/components/atoms";

import { NewConnectionKindPicker } from "./NewConnectionKindPicker";
import {
  CONNECTION_KINDS,
  type ConnectionKind,
} from "./connection-kinds";

export type NewConnectionTabId =
  | "form"
  | "connection-string"
  | "ssh-tunnel"
  | "ssl";

export const NEW_CONNECTION_TABS: ReadonlyArray<{
  id: NewConnectionTabId;
  label: string;
}> = [
  { id: "form", label: "Formulário" },
  { id: "connection-string", label: "Connection string" },
  { id: "ssh-tunnel", label: "SSH tunnel" },
  { id: "ssl", label: "SSL" },
];

const KIND_SUB_LABEL: Record<ConnectionKind, string> = {
  postgres: "Suporta versões 11+. SSH tunnel disponível.",
  mysql: "Suporta MySQL 5.7+ / MariaDB 10.3+.",
  mongo: "MongoDB 4.4+. Driver oficial.",
  bigquery: "Auth via service account JSON.",
  grpc: "Carrega proto via reflexão ou arquivo.",
  graphql: "Endpoint GraphQL com introspecção.",
  http: "Base URL para chamadas HTTP / REST.",
  ws: "WebSocket bidirecional.",
  shell: "Comandos shell em sessão local.",
};

export interface NewConnectionModalProps {
  open: boolean;
  /** Initial selected kind. Defaults to "postgres" (canvas spec). */
  initialKind?: ConnectionKind;
  /** Controlled selected kind (Phase 3). When supplied, the picker
   * routes selection up via `onKindChange` instead of holding local
   * state — lets the consumer patch the kind from a connection-string
   * paste. */
  kind?: ConnectionKind;
  onKindChange?: (next: ConnectionKind) => void;
  /** Controlled active tab (Phase 3). When supplied, the modal calls
   * `onTabChange` instead of holding local state. */
  activeTab?: NewConnectionTabId;
  onTabChange?: (next: NewConnectionTabId) => void;
  /** Called when the user dismisses (Esc, overlay click, Cancel). */
  onCancel: () => void;
  /** Save dispatch — Phase 1 stub; phases 2+3 wire form state. */
  onSave?: (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => void | Promise<void>;
  /** Test dispatch — Phase 1 stub. */
  onTest?: (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => void | Promise<void>;
  /** Renders the active tab's body. Phase 1 ships a placeholder
   * when omitted; phases 2+3 inject the real panels. */
  renderTabBody?: (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => ReactNode;
  /** Disables Save (e.g. invalid form). */
  saveDisabled?: boolean;
}

export function NewConnectionModal({
  open,
  initialKind = "postgres",
  kind: kindProp,
  onKindChange,
  activeTab: activeTabProp,
  onTabChange,
  onCancel,
  onSave,
  onTest,
  renderTabBody,
  saveDisabled = false,
}: NewConnectionModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null);
  const [internalKind, setInternalKind] =
    useState<ConnectionKind>(initialKind);
  const [internalTab, setInternalTab] =
    useState<NewConnectionTabId>("form");
  const selectedKind = kindProp ?? internalKind;
  const activeTab = activeTabProp ?? internalTab;
  const setSelectedKind = (next: ConnectionKind) => {
    if (kindProp === undefined) setInternalKind(next);
    onKindChange?.(next);
  };
  const setActiveTab = (next: NewConnectionTabId) => {
    if (activeTabProp === undefined) setInternalTab(next);
    onTabChange?.(next);
  };

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onCancel]);

  if (!open) return null;

  const meta = CONNECTION_KINDS[selectedKind];

  function handleOverlayClick(e: React.MouseEvent) {
    if (e.target === overlayRef.current) onCancel();
  }

  const tabItems: TabItem[] = NEW_CONNECTION_TABS.map((t) => ({
    id: t.id,
    label: t.label,
  }));

  return (
    <Portal>
      <Box
        ref={overlayRef}
        data-testid="new-connection-modal-overlay"
        position="fixed"
        inset={0}
        bg="blackAlpha.600"
        zIndex={1000}
        display="flex"
        alignItems="center"
        justifyContent="center"
        onClick={handleOverlayClick}
      >
        <Box
          data-testid="new-connection-modal"
          bg="bg"
          borderWidth="1px"
          borderColor="border"
          borderRadius="10px"
          shadow="2xl"
          w="880px"
          maxW="92vw"
          h="660px"
          maxH="92vh"
          overflow="hidden"
          display="grid"
          gridTemplateColumns="220px 1fr"
          onClick={(e) => e.stopPropagation()}
        >
          <NewConnectionKindPicker
            selectedKind={selectedKind}
            onSelectKind={setSelectedKind}
          />

          <Flex direction="column" minW={0} h="full" overflow="hidden">
            <ModalHeader
              Icon={meta.Icon}
              iconColor={`oklch(${meta.hue})`}
              label={meta.label}
              sub={KIND_SUB_LABEL[selectedKind]}
            />

            <Tabbar
              data-testid="new-connection-tabs"
              tabs={tabItems}
              activeId={activeTab}
              onSelect={(id) => setActiveTab(id as NewConnectionTabId)}
              px={5}
            />

            <Box
              data-testid="new-connection-tab-body"
              flex={1}
              minH={0}
              overflowY="auto"
              p={5}
            >
              {renderTabBody ? (
                renderTabBody({ kind: selectedKind, tab: activeTab })
              ) : (
                <TabPlaceholder tab={activeTab} />
              )}
            </Box>

            <ModalFooter
              saveDisabled={saveDisabled}
              onSave={
                onSave
                  ? () => onSave({ kind: selectedKind, tab: activeTab })
                  : undefined
              }
              onTest={
                onTest
                  ? () => onTest({ kind: selectedKind, tab: activeTab })
                  : undefined
              }
              onCancel={onCancel}
            />
          </Flex>
        </Box>
      </Box>
    </Portal>
  );
}

function ModalHeader({
  Icon,
  iconColor,
  label,
  sub,
}: {
  Icon: import("react-icons").IconType;
  iconColor: string;
  label: string;
  sub: string;
}) {
  return (
    <Flex
      data-testid="new-connection-form-header"
      align="center"
      gap={3}
      px={5}
      py={4}
      borderBottomWidth="1px"
      borderBottomColor="border"
    >
      <Box
        aria-hidden
        lineHeight={1}
        flexShrink={0}
        color={iconColor}
        display="inline-flex"
        alignItems="center"
        justifyContent="center"
      >
        <Icon size={26} />
      </Box>
      <Box flex={1} minW={0}>
        <Text
          fontFamily="serif"
          fontSize="22px"
          fontWeight={500}
          color="fg"
          truncate
        >
          {label}
        </Text>
        <Text fontSize="11px" color="fg.muted" truncate>
          {sub}
        </Text>
      </Box>
      <Box
        data-testid="new-connection-paste-hint"
        fontSize="11px"
        color="fg.muted"
        bg="bg.muted"
        borderWidth="1px"
        borderColor="border"
        borderRadius="999px"
        px={3}
        py={1}
        flexShrink={0}
      >
        ⌥ Cole uma{" "}
        <Text as="span" fontFamily="mono">
          connection string
        </Text>
      </Box>
    </Flex>
  );
}

function ModalFooter({
  saveDisabled,
  onSave,
  onTest,
  onCancel,
}: {
  saveDisabled: boolean;
  onSave?: () => void;
  onTest?: () => void;
  onCancel: () => void;
}) {
  return (
    <Flex
      data-testid="new-connection-footer"
      borderTopWidth="1px"
      borderTopColor="border"
      px={5}
      py={4}
      align="center"
      gap={2}
    >
      <Btn
        variant="primary"
        data-testid="new-connection-save"
        disabled={saveDisabled || !onSave}
        onClick={onSave}
      >
        Salvar conexão
      </Btn>
      <Btn
        variant="ghost"
        data-testid="new-connection-cancel"
        onClick={onCancel}
      >
        Cancelar
      </Btn>
      <Box flex={1} />
      <Btn
        variant="ghost"
        data-testid="new-connection-test"
        disabled={!onTest}
        onClick={onTest}
      >
        <LuPlay size={12} /> Testar conexão
      </Btn>
    </Flex>
  );
}

function TabPlaceholder({ tab }: { tab: NewConnectionTabId }) {
  const label = NEW_CONNECTION_TABS.find((t) => t.id === tab)?.label ?? tab;
  return (
    <Box
      data-testid={`new-connection-placeholder-${tab}`}
      fontSize="12px"
      color="fg.subtle"
    >
      Conteúdo da aba “{label}” virá aqui.
    </Box>
  );
}
