// Epic 48 Story 04 — branch picker UI.
//
// Pure presentational. Consumer fetches `BranchInfo[]` via the
// existing `git_branch_list_cmd` (already shipped in 5a6e217's
// scaffold dependencies) and feeds it in. The picker handles:
//
//   - Filter-as-you-type search across local + remote branches
//   - Current branch marked + auto-skipped from selection
//   - "New branch…" inline form (consumer wires onCreateBranch
//     to git_checkout_b once that Tauri cmd ships)
//   - Click → onSelectBranch(name); consumer routes to
//     `git_checkout` (new cmd) and handles the dirty-state stash
//     prompt itself

import { useState } from "react";
import { Box, Flex, Text } from "@chakra-ui/react";

import { Btn, Input } from "@/components/atoms";
import type { BranchInfo } from "@/lib/tauri/git";

export interface GitBranchPickerProps {
  branches: ReadonlyArray<BranchInfo>;
  /** True while a checkout is in flight. Disables the picker
   *  inputs to prevent double-firing while git is busy. */
  busy?: boolean;
  onSelectBranch?: (branch: BranchInfo) => void;
  /** Fires with the new branch name typed by the user. Consumer
   *  routes to `git_checkout_b`. */
  onCreateBranch?: (name: string) => void;
}

export function GitBranchPicker({
  branches,
  busy,
  onSelectBranch,
  onCreateBranch,
}: GitBranchPickerProps) {
  const [filter, setFilter] = useState("");
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  const trimmed = filter.trim().toLowerCase();
  const filtered = trimmed
    ? branches.filter((b) => b.name.toLowerCase().includes(trimmed))
    : branches;
  const local = filtered.filter((b) => !b.remote);
  const remote = filtered.filter((b) => b.remote);

  return (
    <Box
      data-testid="git-branch-picker"
      data-busy={busy || undefined}
      data-creating={creating || undefined}
      px={3}
      py={3}
      bg="bg.subtle"
      borderWidth="1px"
      borderColor="border"
      borderRadius="6px"
      minW="280px"
    >
      {!creating && (
        <Box mb={2}>
          <Input
            data-testid="git-branch-picker-filter"
            placeholder="Filter branches…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            disabled={busy}
          />
        </Box>
      )}

      {creating ? (
        <CreateForm
          name={newName}
          onChange={setNewName}
          onSubmit={() => {
            const v = newName.trim();
            if (v.length === 0) return;
            onCreateBranch?.(v);
            setNewName("");
            setCreating(false);
          }}
          onCancel={() => {
            setNewName("");
            setCreating(false);
          }}
          busy={busy}
        />
      ) : (
        <>
          {local.length > 0 && (
            <Section testId="git-branch-picker-local" label="Local">
              {local.map((b) => (
                <Row
                  key={b.name}
                  branch={b}
                  busy={busy}
                  onSelect={onSelectBranch}
                />
              ))}
            </Section>
          )}
          {remote.length > 0 && (
            <Section testId="git-branch-picker-remote" label="Remote">
              {remote.map((b) => (
                <Row
                  key={b.name}
                  branch={b}
                  busy={busy}
                  onSelect={onSelectBranch}
                />
              ))}
            </Section>
          )}
          {filtered.length === 0 && (
            <Text
              data-testid="git-branch-picker-empty"
              fontSize="11px"
              color="fg.subtle"
              py={2}
            >
              No branches match "{filter}".
            </Text>
          )}
          {onCreateBranch && (
            <Btn
              data-testid="git-branch-picker-new"
              variant="ghost"
              onClick={() => setCreating(true)}
              disabled={busy}
            >
              + New branch…
            </Btn>
          )}
        </>
      )}
    </Box>
  );
}

function Section({
  testId,
  label,
  children,
}: {
  testId: string;
  label: string;
  children: React.ReactNode;
}) {
  return (
    <Box data-testid={testId} mb={2}>
      <Text
        as="div"
        fontFamily="mono"
        fontSize="10px"
        textTransform="uppercase"
        color="fg.subtle"
        mb={1}
      >
        {label}
      </Text>
      {children}
    </Box>
  );
}

function Row({
  branch,
  busy,
  onSelect,
}: {
  branch: BranchInfo;
  busy?: boolean;
  onSelect?: (b: BranchInfo) => void;
}) {
  const interactive = !!onSelect && !branch.current && !busy;
  return (
    <Flex
      as={interactive ? "button" : "div"}
      data-testid={`git-branch-picker-row-${branch.name}`}
      data-current={branch.current || undefined}
      align="center"
      gap={2}
      px={2}
      py={1}
      w="100%"
      textAlign="left"
      cursor={interactive ? "pointer" : undefined}
      onClick={interactive ? () => onSelect(branch) : undefined}
      _hover={interactive ? { bg: "bg.muted" } : undefined}
      borderRadius="4px"
    >
      <Text
        as="span"
        fontFamily="mono"
        fontSize="10px"
        color={branch.current ? "brand.fg" : "fg.subtle"}
        flexShrink={0}
        w="14px"
        textAlign="center"
      >
        {branch.current ? "●" : ""}
      </Text>
      <Text
        as="span"
        fontFamily="mono"
        fontSize="11px"
        color={branch.current ? "fg" : "fg.muted"}
        flex={1}
        truncate
      >
        {branch.name}
      </Text>
    </Flex>
  );
}

function CreateForm({
  name,
  onChange,
  onSubmit,
  onCancel,
  busy,
}: {
  name: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
  busy?: boolean;
}) {
  return (
    <Box data-testid="git-branch-picker-create-form">
      <Text
        as="div"
        fontFamily="mono"
        fontSize="10px"
        textTransform="uppercase"
        color="fg.subtle"
        mb={1}
      >
        New branch
      </Text>
      <Input
        data-testid="git-branch-picker-create-input"
        placeholder="branch-name"
        value={name}
        onChange={(e) => onChange(e.target.value)}
        disabled={busy}
        onKeyDown={(e) => {
          if (e.key === "Enter") onSubmit();
          else if (e.key === "Escape") onCancel();
        }}
      />
      <Flex gap={2} mt={2}>
        <Btn
          data-testid="git-branch-picker-create-submit"
          variant="primary"
          onClick={onSubmit}
          disabled={busy || name.trim().length === 0}
        >
          Create
        </Btn>
        <Btn
          data-testid="git-branch-picker-create-cancel"
          variant="ghost"
          onClick={onCancel}
          disabled={busy}
        >
          Cancel
        </Btn>
      </Flex>
    </Box>
  );
}
