// V4 cenário 3 — wires <NewConnectionModal /> form state.
//
// Lifts kind + active tab + Postgres form value + SSL value, dispatches
// the right child per tab, and submits createConnection on Save.

import { useState } from "react";

import {
  createConnection,
  type CreateConnectionInput,
} from "@/lib/tauri/connections";

import {
  NewConnectionModal,
  type NewConnectionTabId,
} from "./NewConnectionModal";
import { NewConnectionFormTab, EMPTY_POSTGRES_VALUE } from "./NewConnectionFormTab";
import type { PostgresFormValue } from "./NewConnectionFormTab";
import { NewConnectionStringTab } from "./NewConnectionStringTab";
import { NewConnectionSshTab } from "./NewConnectionSshTab";
import { NewConnectionSslTab, EMPTY_SSL_VALUE } from "./NewConnectionSslTab";
import type { SslFormValue } from "./NewConnectionSslTab";
import type { ConnectionKind } from "./connection-kinds";

interface NewConnectionModalContainerProps {
  open: boolean;
  onClose: () => void;
  /** Called after a successful createConnection so the parent can
   * refresh the list. */
  onCreated: () => void;
}

const SUPPORTED_DRIVERS: ReadonlySet<ConnectionKind> = new Set([
  "postgres",
  "mysql",
]);

export function NewConnectionModalContainer({
  open,
  onClose,
  onCreated,
}: NewConnectionModalContainerProps) {
  const [kind, setKind] = useState<ConnectionKind>("postgres");
  const [tab, setTab] = useState<NewConnectionTabId>("form");
  const [form, setForm] = useState<PostgresFormValue>(EMPTY_POSTGRES_VALUE);
  const [ssl, setSsl] = useState<SslFormValue>(EMPTY_SSL_VALUE);

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

  const handleSave = async () => {
    if (!SUPPORTED_DRIVERS.has(kind)) return;
    const portNum = Number(form.port);
    const input: CreateConnectionInput = {
      name: form.name.trim(),
      driver: kind as "postgres" | "mysql",
      host: form.host.trim() || undefined,
      port: Number.isFinite(portNum) && portNum > 0 ? portNum : undefined,
      database_name: form.database.trim() || undefined,
      username: form.username.trim() || undefined,
      password: form.password || undefined,
      ssl_mode: ssl.mode || undefined,
    };
    if (!input.name) return;
    await createConnection(input);
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
      onKindChange={setKind}
      activeTab={tab}
      onTabChange={setTab}
      renderTabBody={renderTabBody}
      saveDisabled={saveDisabled}
      onSave={handleSave}
      onCancel={handleClose}
    />
  );
}
