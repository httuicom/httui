// "Overridden locally" badge for Settings → Workspace fields. V3
// cenário 3. Hover reveals the source path. Stays a separate
// component because the same badge is used across all fields and
// future verticals (envs, connections) will likely reuse it too.

import { Box } from "@chakra-ui/react";

interface OverrideBadgeProps {
  label: string;
  tooltip: string;
  "data-testid"?: string;
}

export function OverrideBadge(props: OverrideBadgeProps) {
  const { label, tooltip } = props;
  return (
    <Box
      as="span"
      data-testid={props["data-testid"]}
      title={tooltip}
      display="inline-flex"
      alignItems="center"
      px={1.5}
      py={0.5}
      borderRadius="sm"
      bg="bg.muted"
      color="fg.muted"
      fontSize="xs"
      fontWeight={500}
      borderWidth="1px"
      borderColor="border"
      cursor="help"
    >
      {label}
    </Box>
  );
}
