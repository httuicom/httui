// Workspace tab — vault-committed defaults + local overrides. V3
// cenário 2 + 3.
//
// Reads via getWorkspaceConfigWithSources so each field comes paired
// with its origin (`workspace` | `local`). Writes via
// setWorkspaceConfig — always to the base file, never `.local.toml`
// (audit-003 contract enforced by WorkspaceStore).

import { useCallback, useEffect, useState } from "react";
import {
  Box,
  Field,
  Flex,
  Input,
  Spinner,
  Text,
  VStack,
} from "@chakra-ui/react";

import {
  getWorkspaceConfigWithSources,
  setWorkspaceConfig,
  type WorkspaceDefaults,
  type WorkspaceFieldSource,
  type WorkspaceSources,
} from "@/lib/tauri/commands";
import { useWorkspaceStore } from "@/stores/workspace";

import { OverrideBadge } from "./OverrideBadge";

interface FieldDef {
  key: keyof WorkspaceDefaults & keyof WorkspaceSources;
  label: string;
  helper: string;
  placeholder: string;
}

const FIELDS: ReadonlyArray<FieldDef> = [
  {
    key: "environment",
    label: "Default environment",
    helper: "Which environment to activate when the vault opens.",
    placeholder: "staging",
  },
  {
    key: "git_remote",
    label: "Git remote",
    helper: "Remote name used for sync (typically `origin`).",
    placeholder: "origin",
  },
  {
    key: "display_name",
    label: "Vault display name",
    helper:
      "Human-friendly label shown in the workspace switcher. Falls back to the directory name.",
    placeholder: "Payments",
  },
];

export function SettingsWorkspaceTab() {
  const vaultPath = useWorkspaceStore((s) => s.vaultPath);
  const [defaults, setDefaults] = useState<WorkspaceDefaults | null>(null);
  const [sources, setSources] = useState<WorkspaceSources | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!vaultPath) {
      setDefaults(null);
      setSources(null);
      return;
    }
    let cancelled = false;
    getWorkspaceConfigWithSources(vaultPath)
      .then((res) => {
        if (cancelled) return;
        setDefaults(res.defaults);
        setSources(res.sources);
        setError(null);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [vaultPath]);

  const handleCommit = useCallback(
    async (key: FieldDef["key"], value: string) => {
      if (!vaultPath || !defaults) return;
      const trimmed = value.trim();
      const next: WorkspaceDefaults = {
        ...defaults,
        [key]: trimmed.length > 0 ? trimmed : null,
      };
      setDefaults(next);
      try {
        await setWorkspaceConfig(vaultPath, next);
        // Re-fetch sources so the badge flips back to `workspace`
        // when the user overwrote a previously local-overridden field
        // via the UI (write hits base only).
        const fresh = await getWorkspaceConfigWithSources(vaultPath);
        setDefaults(fresh.defaults);
        setSources(fresh.sources);
      } catch (e: unknown) {
        setError(String(e));
      }
    },
    [defaults, vaultPath],
  );

  if (!vaultPath) {
    return (
      <Box
        data-testid="settings-workspace-empty"
        p={4}
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        bg="bg.subtle"
      >
        <Text fontSize="sm" color="fg.muted">
          Open a vault to edit workspace defaults.
        </Text>
      </Box>
    );
  }

  if (!defaults || !sources) {
    return (
      <Flex
        data-testid="settings-workspace-loading"
        align="center"
        gap={2}
        p={4}
      >
        <Spinner size="sm" />
        <Text fontSize="sm" color="fg.muted">
          Loading workspace config…
        </Text>
      </Flex>
    );
  }

  return (
    <VStack align="stretch" gap={5} data-testid="settings-workspace-tab">
      {error && (
        <Box
          data-testid="settings-workspace-error"
          p={3}
          borderWidth="1px"
          borderColor="red.300"
          borderRadius="md"
          bg="red.50"
          color="red.900"
        >
          <Text fontSize="xs">{error}</Text>
        </Box>
      )}
      {FIELDS.map((f) => {
        const value = (defaults[f.key] ?? "") as string;
        const source: WorkspaceFieldSource = sources[f.key];
        return (
          <FieldRow
            key={f.key}
            def={f}
            value={value}
            source={source}
            onCommit={(v) => handleCommit(f.key, v)}
          />
        );
      })}
    </VStack>
  );
}

interface FieldRowProps {
  def: FieldDef;
  value: string;
  source: WorkspaceFieldSource;
  onCommit: (value: string) => void;
}

function FieldRow({ def, value, source, onCommit }: FieldRowProps) {
  const [draft, setDraft] = useState(value);

  // Keep draft in sync when defaults reload from disk (e.g. after
  // commit). Avoid clobbering an in-flight edit.
  useEffect(() => {
    setDraft(value);
  }, [value]);

  return (
    <Field.Root>
      <Flex align="center" gap={2}>
        <Field.Label fontSize="sm" fontWeight={600} color="fg">
          {def.label}
        </Field.Label>
        {source === "local" && (
          <OverrideBadge
            data-testid={`override-badge-${def.key}`}
            label="overridden locally"
            tooltip="set in .httui/workspace.local.toml"
          />
        )}
      </Flex>
      <Field.HelperText fontSize="xs" color="fg.muted" mt={0}>
        {def.helper}
      </Field.HelperText>
      <Input
        size="sm"
        mt={2}
        value={draft}
        placeholder={def.placeholder}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => {
          if (draft !== value) onCommit(draft);
        }}
      />
    </Field.Root>
  );
}
