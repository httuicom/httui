// New-connection modal shell: 880×660, two-column (220px kind picker + 1fr form).
// Tab strip uses the `Tabbar` atom. Pure presentational — save/test/cancel
// dispatched up via callbacks.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { Box, Flex, Portal, Text } from "@chakra-ui/react";
import { LuPlay } from "react-icons/lu";

import { Btn, Tabbar, type TabItem } from "@/components/atoms";

import { NewConnectionKindPicker } from "./NewConnectionKindPicker";
import {
  CONNECTION_KINDS,
  tabsForKind,
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
  { id: "form", label: "Form" },
  { id: "connection-string", label: "Connection string" },
  { id: "ssh-tunnel", label: "SSH tunnel" },
  { id: "ssl", label: "SSL" },
];

const KIND_SUB_LABEL: Record<ConnectionKind, string> = {
  postgres: "Supports versions 11+. SSH tunnel available.",
  mysql: "Supports MySQL 5.7+ / MariaDB 10.3+.",
  sqlite: "Local file-based database. No host or credentials.",
  mongo: "MongoDB 4.4+. Official driver.",
  bigquery: "Auth via service account JSON.",
  grpc: "Loads proto via reflection or file.",
  graphql: "GraphQL endpoint with introspection.",
  http: "Base URL for HTTP / REST calls.",
  ws: "Bidirectional WebSocket.",
  shell: "Shell commands in a local session.",
};

export interface NewConnectionModalProps {
  open: boolean;
  initialKind?: ConnectionKind;
  /** Controlled kind; routes selection up so a pasted connection string can patch it. */
  kind?: ConnectionKind;
  onKindChange?: (next: ConnectionKind) => void;
  /** Controlled active tab. */
  activeTab?: NewConnectionTabId;
  onTabChange?: (next: NewConnectionTabId) => void;
  onCancel: () => void;
  onSave?: (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => void | Promise<void>;
  onTest?: (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => void | Promise<void>;
  /** Render the active tab body; omit for a placeholder. */
  renderTabBody?: (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => ReactNode;
  saveDisabled?: boolean;
  /** Kinds outside this list render a "Coming soon" state with tabs/footer hidden. */
  supportedKinds?: ReadonlyArray<ConnectionKind>;
  /** "edit" locks the kind picker; title and Save label adapt. */
  mode?: "create" | "edit";
  editingName?: string;
  /** Save/IPC failure message. Rendered as an alert above the footer;
   * the consumer keeps the modal open so the user can retry. */
  error?: string | null;
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
  supportedKinds,
  mode = "create",
  editingName,
  error,
}: NewConnectionModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null);
  const [internalKind, setInternalKind] = useState<ConnectionKind>(initialKind);
  const [internalTab, setInternalTab] = useState<NewConnectionTabId>("form");
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
  const isSupported =
    supportedKinds === undefined || supportedKinds.includes(selectedKind);

  function handleOverlayClick(e: React.MouseEvent) {
    if (e.target === overlayRef.current) onCancel();
  }

  const allowedTabs = new Set(tabsForKind(selectedKind));
  const tabItems: TabItem[] = NEW_CONNECTION_TABS.filter((t) =>
    allowedTabs.has(t.id),
  ).map((t) => ({ id: t.id, label: t.label }));
  const showTabbar = tabItems.length > 1;
  // Fall back to "form" if the active tab isn't valid for this kind (e.g. SSL → SQLite).
  const effectiveTab = allowedTabs.has(activeTab) ? activeTab : "form";

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
            disabled={mode === "edit"}
            mode={mode}
          />

          <Flex direction="column" minW={0} h="full" overflow="hidden">
            <ModalHeader
              Icon={meta.Icon}
              iconColor={`oklch(${meta.hue})`}
              label={
                mode === "edit" && editingName
                  ? `Edit ${editingName}`
                  : meta.label
              }
              sub={KIND_SUB_LABEL[selectedKind]}
            />

            {isSupported ? (
              <>
                {showTabbar && (
                  <Tabbar
                    data-testid="new-connection-tabs"
                    tabs={tabItems}
                    activeId={effectiveTab}
                    onSelect={(id) => setActiveTab(id as NewConnectionTabId)}
                    px={5}
                  />
                )}

                <Box
                  data-testid="new-connection-tab-body"
                  flex={1}
                  minH={0}
                  overflowY="auto"
                  p={5}
                >
                  {renderTabBody ? (
                    renderTabBody({ kind: selectedKind, tab: effectiveTab })
                  ) : (
                    <TabPlaceholder tab={effectiveTab} />
                  )}
                </Box>

                <ErrorBanner message={error} />

                <ModalFooter
                  saveDisabled={saveDisabled}
                  saveLabel={
                    mode === "edit" ? "Save changes" : "Save connection"
                  }
                  onSave={
                    onSave
                      ? () => onSave({ kind: selectedKind, tab: effectiveTab })
                      : undefined
                  }
                  onTest={
                    onTest
                      ? () => onTest({ kind: selectedKind, tab: effectiveTab })
                      : undefined
                  }
                  onCancel={onCancel}
                />
              </>
            ) : (
              <ComingSoonState kindLabel={meta.label} onCancel={onCancel} />
            )}
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
        ⌥ Paste a{" "}
        <Text as="span" fontFamily="mono">
          connection string
        </Text>
      </Box>
    </Flex>
  );
}

function ModalFooter({
  saveDisabled,
  saveLabel = "Save connection",
  onSave,
  onTest,
  onCancel,
}: {
  saveDisabled: boolean;
  saveLabel?: string;
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
        {saveLabel}
      </Btn>
      <Btn
        variant="ghost"
        data-testid="new-connection-cancel"
        onClick={onCancel}
      >
        Cancel
      </Btn>
      <Box flex={1} />
      <Btn
        variant="ghost"
        data-testid="new-connection-test"
        disabled={!onTest}
        onClick={onTest}
      >
        <LuPlay size={12} /> Test connection
      </Btn>
    </Flex>
  );
}

function ComingSoonState({
  kindLabel,
  onCancel,
}: {
  kindLabel: string;
  onCancel: () => void;
}) {
  return (
    <Flex
      data-testid="new-connection-coming-soon"
      direction="column"
      align="center"
      justify="center"
      gap={3}
      flex={1}
      px={6}
      textAlign="center"
    >
      <Text fontFamily="serif" fontSize="20px" color="fg" fontWeight={500}>
        {kindLabel} — coming soon
      </Text>
      <Text fontSize="13px" color="fg.muted" maxW="360px">
        Support for {kindLabel} connections lands in a future release. Pick
        another kind on the left or close this dialog.
      </Text>
      <Box mt={2}>
        <Btn variant="ghost" onClick={onCancel}>
          Close
        </Btn>
      </Box>
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
      Tab “{label}” content coming soon.
    </Box>
  );
}

function ErrorBanner({ message }: { message?: string | null }) {
  if (!message) return null;
  return (
    <Text
      data-testid="new-connection-error"
      role="alert"
      px={5}
      py={2}
      fontSize="12px"
      color="red.fg"
      borderTopWidth="1px"
      borderTopColor="border"
    >
      {message}
    </Text>
  );
}
