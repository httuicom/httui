// wires <NewConnectionModal /> form state.
//
// Lifts kind + active tab + Postgres form value + SSL value, dispatches
// the right child per tab, and submits createConnection on Save.

import { useEffect, useState } from "react";

import {
  createConnection,
  updateConnection,
  type Connection,
  type CreateConnectionInput,
  type UpdateConnectionInput,
} from "@/lib/tauri/connections";

import {
  NewConnectionModal,
  type NewConnectionTabId,
} from "./NewConnectionModal";
import {
  NewConnectionFormTab,
  EMPTY_POSTGRES_VALUE,
  emptyFormValueForKind,
} from "./NewConnectionFormTab";
import type { PostgresFormValue } from "./NewConnectionFormTab";
import { NewConnectionStringTab } from "./NewConnectionStringTab";
import { NewConnectionSshTab } from "./NewConnectionSshTab";
import { NewConnectionSslTab, EMPTY_SSL_VALUE } from "./NewConnectionSslTab";
import type { SslFormValue } from "./NewConnectionSslTab";
import {
  SUPPORTED_NEW_CONNECTION_KINDS,
  type ConnectionKind,
} from "./connection-kinds";

interface NewConnectionModalContainerProps {
  open: boolean;
  onClose: () => void;
  /** Called after a successful create / update so the parent can
   * refresh the list. */
  onCreated: () => void;
  /** When set, modal opens in "edit" mode pre-populated with this
   * connection's fields. Save dispatches updateConnection instead
   * of createConnection. */
  editing?: Connection | null;
}

const SUPPORTED_DRIVERS: ReadonlySet<ConnectionKind> = new Set(
  SUPPORTED_NEW_CONNECTION_KINDS,
);

export function NewConnectionModalContainer({
  open,
  onClose,
  onCreated,
  editing,
}: NewConnectionModalContainerProps) {
  const [kind, setKind] = useState<ConnectionKind>("postgres");
  const [tab, setTab] = useState<NewConnectionTabId>("form");
  const [form, setForm] = useState<PostgresFormValue>(EMPTY_POSTGRES_VALUE);
  const [ssl, setSsl] = useState<SslFormValue>(EMPTY_SSL_VALUE);

  const isEdit = Boolean(editing);

  // Hydrate form when entering edit mode (or when `editing` switches).
  useEffect(() => {
    if (!editing || !open) return;
    const drvKind: ConnectionKind =
      editing.driver === "mysql"
        ? "mysql"
        : editing.driver === "sqlite"
          ? "sqlite"
          : "postgres";
    setKind(drvKind);
    setForm({
      name: editing.name,
      host: editing.host ?? "",
      port: editing.port !== null ? String(editing.port) : "",
      database: editing.database_name ?? "",
      username: editing.username ?? "",
      password: "", // never echoed back from keychain — leave blank
    });
    setSsl({
      mode: (editing.ssl_mode ?? "") as SslFormValue["mode"],
      rootCertPath: "",
      clientCertPath: "",
      clientKeyPath: "",
    });
    setTab("form");
  }, [editing, open]);

  const reset = () => {
    setForm(EMPTY_POSTGRES_VALUE);
    setSsl(EMPTY_SSL_VALUE);
    setKind("postgres");
    setTab("form");
  };

  const handleClose = () => {
    reset();
    onClose();
  };

  /** When the user picks a different kind, swap the kind defaults
   * (port mostly) UNLESS they've started typing a name — preserves
   * mid-edit state. */
  const handleKindChange = (next: ConnectionKind) => {
    setKind(next);
    if (form.name.trim().length === 0) {
      setForm(emptyFormValueForKind(next));
    }
  };

  const handleSave = async () => {
    if (!SUPPORTED_DRIVERS.has(kind)) return;
    if (form.name.trim().length === 0) return;

    if (isEdit && editing) {
      // Edit path: only send fields that changed shape sense; password
      // is only sent when the user actually typed something (otherwise
      // the keychain value stays).
      const portNum = Number(form.port);
      const update: UpdateConnectionInput = {
        host: form.host.trim() || undefined,
        port: Number.isFinite(portNum) && portNum > 0 ? portNum : undefined,
        database_name: form.database.trim() || undefined,
        username: form.username.trim() || undefined,
        ssl_mode: ssl.mode || undefined,
      };
      if (form.password.length > 0) update.password = form.password;
      await updateConnection(editing.id, update);
    } else {
      let input: CreateConnectionInput;
      if (kind === "sqlite") {
        input = {
          name: form.name.trim(),
          driver: "sqlite",
          database_name: form.database.trim() || undefined,
        };
      } else {
        const portNum = Number(form.port);
        input = {
          name: form.name.trim(),
          driver: kind as "postgres" | "mysql",
          host: form.host.trim() || undefined,
          port: Number.isFinite(portNum) && portNum > 0 ? portNum : undefined,
          database_name: form.database.trim() || undefined,
          username: form.username.trim() || undefined,
          password: form.password || undefined,
          ssl_mode: ssl.mode || undefined,
        };
      }
      await createConnection(input);
    }
    reset();
    onCreated();
    onClose();
  };

  const renderTabBody = (args: {
    kind: ConnectionKind;
    tab: NewConnectionTabId;
  }) => {
    switch (args.tab) {
      case "form":
        return (
          <NewConnectionFormTab
            kind={args.kind}
            value={form}
            onChange={setForm}
          />
        );
      case "connection-string":
        return (
          <NewConnectionStringTab
            kind={args.kind}
            onApply={({ kind: parsedKind, value, ssl: parsedSsl }) => {
              setKind(parsedKind);
              setForm(value);
              setSsl(parsedSsl);
              setTab("form");
            }}
          />
        );
      case "ssh-tunnel":
        return <NewConnectionSshTab />;
      case "ssl":
        return <NewConnectionSslTab value={ssl} onChange={setSsl} />;
    }
  };

  const saveDisabled =
    !SUPPORTED_DRIVERS.has(kind) || form.name.trim().length === 0;

  return (
    <NewConnectionModal
      open={open}
      kind={kind}
      onKindChange={handleKindChange}
      activeTab={tab}
      onTabChange={setTab}
      renderTabBody={renderTabBody}
      saveDisabled={saveDisabled}
      onSave={handleSave}
      onCancel={handleClose}
      supportedKinds={SUPPORTED_NEW_CONNECTION_KINDS}
      mode={isEdit ? "edit" : "create"}
      editingName={editing?.name}
    />
  );
}
