// Canvas §6 Environments — delete confirmation (Epic 44 Story 04).
//
// Inline destructive-action banner. The user must type the env name
// into the confirm input before the Delete button enables — the
// industry-standard guardrail against accidental clicks. Surfaces
// the var + secret counts so it's clear what's about to be lost.

import { Box, Flex, Text } from "@chakra-ui/react";
import { useState } from "react";
import { LuTriangleAlert } from "react-icons/lu";

import { Btn, Input } from "@/components/atoms";

import type { EnvironmentSummary } from "./envs-meta";

export interface DeleteEnvironmentConfirmProps {
  env: EnvironmentSummary;
  /** Number of secrets that will lose their keychain entries. Optional —
   * when omitted the body just mentions the vars total. */
  secretCount?: number;
  onConfirm?: (filename: string) => void;
  onCancel?: () => void;
}

export function DeleteEnvironmentConfirm({
  env,
  secretCount,
  onConfirm,
  onCancel,
}: DeleteEnvironmentConfirmProps) {
  const [confirmInput, setConfirmInput] = useState("");
  const matches = confirmInput.trim() === env.name;

  return (
    <Box
      data-testid="delete-environment-confirm"
      data-target={env.filename}
      mx={5}
      my={3}
      borderWidth="1px"
      borderColor="red.fg"
      borderRadius="6px"
      bg="bg"
      overflow="hidden"
    >
      <Flex
        align="center"
        gap={2}
        px={4}
        py={2.5}
        bg="red.subtle"
        borderBottomWidth="1px"
        borderBottomColor="red.fg"
        color="red.fg"
      >
        <LuTriangleAlert size={14} />
        <Text fontSize="11px" fontWeight="bold" letterSpacing="0.04em">
          DESTRUCTIVE ACTION — CANNOT BE UNDONE
        </Text>
      </Flex>

      <Box px={4} py={3}>
        <Text
          fontSize="13px"
          fontWeight="bold"
          color="fg"
          mb={1.5}
          data-testid="delete-environment-confirm-heading"
        >
          Delete{" "}
          <Text as="span" fontFamily="mono">
            {env.name}
          </Text>
          ?
        </Text>
        <Text
          fontSize="11px"
          color="fg.muted"
          data-testid="delete-environment-confirm-body"
          lineHeight={1.5}
        >
          This deletes{" "}
          <Text as="span" fontFamily="mono">
            envs/{env.filename}
          </Text>{" "}
          ({env.varCount} {env.varCount === 1 ? "var" : "vars"}
          {secretCount && secretCount > 0 ? (
            <>
              {", including "}
              <Text as="span" fontFamily="mono" color="red.fg">
                {secretCount} {secretCount === 1 ? "secret" : "secrets"}
              </Text>{" "}
              from the keychain
            </>
          ) : null}
          )
          {env.isPersonal
            ? " from your machine (it's gitignored — won't affect the team)"
            : " and any .local.toml siblings on this machine"}
          .
        </Text>

        <Box mt={3}>
          <Text
            as="label"
            fontSize="11px"
            color="fg.muted"
            display="block"
            mb={1}
          >
            Type{" "}
            <Text as="span" fontFamily="mono" color="fg">
              {env.name}
            </Text>{" "}
            to confirm:
          </Text>
          <Input
            data-testid="delete-environment-confirm-input"
            value={confirmInput}
            onChange={(e) => setConfirmInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && matches) {
                e.preventDefault();
                onConfirm?.(env.filename);
              } else if (e.key === "Escape") {
                e.preventDefault();
                onCancel?.();
              }
            }}
            autoFocus
            placeholder={env.name}
          />
        </Box>
      </Box>

      <Flex
        justify="flex-end"
        gap={2}
        px={4}
        py={3}
        borderTopWidth="1px"
        borderTopColor="border"
        bg="bg.subtle"
      >
        <Btn
          variant="ghost"
          data-testid="delete-environment-cancel"
          onClick={onCancel}
        >
          Cancel
        </Btn>
        <Btn
          variant="primary"
          data-testid="delete-environment-confirm-submit"
          onClick={() => onConfirm?.(env.filename)}
          disabled={!matches}
          colorPalette="red"
        >
          Delete
        </Btn>
      </Flex>
    </Box>
  );
}
