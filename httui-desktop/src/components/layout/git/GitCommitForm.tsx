import { Box, Flex, Text, Textarea } from "@chakra-ui/react";

import { Btn } from "@/components/atoms";
import { Checkbox } from "@/components/ui/checkbox";

import { pluralizeFiles, validateCommitMessage } from "./git-commit-validate";

export interface GitCommitFormProps {
  message: string;
  amend: boolean;
  /** Number of files staged. The Commit button is disabled when 0. */
  stagedCount: number;
  /** Disabled while a commit is in flight to prevent double-submit. */
  busy?: boolean;
  onMessageChange: (next: string) => void;
  onAmendChange: (next: boolean) => void;
  onCommit: (input: { message: string; amend: boolean }) => void;
}

export function GitCommitForm({
  message,
  amend,
  stagedCount,
  busy,
  onMessageChange,
  onAmendChange,
  onCommit,
}: GitCommitFormProps) {
  const validation = validateCommitMessage(message);
  const disabled = !validation.valid || stagedCount === 0 || !!busy;

  return (
    <Box
      data-testid="git-commit-form"
      data-busy={busy || undefined}
      data-disabled={disabled || undefined}
      px={3}
      py={2}
      borderTopWidth="1px"
      borderTopColor="border"
      bg="bg.subtle"
    >
      <Textarea
        data-testid="git-commit-form-message"
        placeholder="Commit message — subject on the first line, blank line, then body."
        value={message}
        onChange={(e) => onMessageChange(e.target.value)}
        rows={3}
        fontFamily="mono"
        fontSize="11px"
        resize="vertical"
        bg="bg"
        borderColor="border"
      />
      {validation.errors.map((err, i) => (
        <Text
          key={i}
          data-testid={`git-commit-form-error-${i}`}
          data-role="error"
          fontSize="10px"
          color="error"
          mt={1}
        >
          {err}
        </Text>
      ))}
      <Flex align="center" gap={3} mt={2}>
        <Checkbox
          data-testid="git-commit-form-amend"
          checked={amend}
          onCheckedChange={(d) => onAmendChange(!!d.checked)}
          disabled={busy}
        >
          <Text fontSize="11px" color="fg.muted">
            Amend last
          </Text>
        </Checkbox>
        <Box flex={1} />
        <Text
          data-testid="git-commit-form-summary"
          fontSize="10px"
          color="fg.subtle"
          flexShrink={0}
        >
          {pluralizeFiles(stagedCount)} staged
        </Text>
        <Btn
          variant={disabled ? "ghost" : "primary"}
          data-testid="git-commit-form-submit"
          disabled={disabled}
          onClick={() => onCommit({ message: message.trim(), amend })}
        >
          {amend ? "Amend" : "Commit"}
        </Btn>
      </Flex>
    </Box>
  );
}
