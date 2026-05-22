import { Box, HStack, chakra } from "@chakra-ui/react";

import { Dot } from "@/components/atoms";
import { useEnvironmentStore } from "@/stores/environment";

const Cell = chakra("button");

const PROD_PREFIX_RE = /^prod/i;

export function SegmentedEnvSwitcher() {
  const environments = useEnvironmentStore((s) => s.environments);
  const activeEnvironment = useEnvironmentStore((s) => s.activeEnvironment);
  const switchEnvironment = useEnvironmentStore((s) => s.switchEnvironment);

  if (environments.length === 0) {
    return (
      <Box
        data-atom="env-switcher"
        data-empty="true"
        h="24px"
        px={2}
        display="inline-flex"
        alignItems="center"
        fontSize="11px"
        color="fg.subtle"
        fontFamily="mono"
      >
        no env
      </Box>
    );
  }

  return (
    <HStack
      data-atom="env-switcher"
      role="tablist"
      aria-label="Environment"
      gap={0}
      h="24px"
      borderWidth="1px"
      borderColor="border"
      borderRadius="4px"
      overflow="hidden"
      flexShrink={0}
    >
      {environments.map((env, idx) => {
        const active = activeEnvironment?.id === env.id;
        const isProd = PROD_PREFIX_RE.test(env.name);
        return (
          <Cell
            type="button"
            key={env.id}
            role="tab"
            aria-selected={active}
            data-env-id={env.id}
            data-env-name={env.name}
            data-active={active ? "true" : "false"}
            onClick={() => {
              if (!active) void switchEnvironment(env.id);
            }}
            h="24px"
            px={3}
            gap={1.5}
            display="inline-flex"
            alignItems="center"
            fontFamily="mono"
            fontSize="11px"
            fontWeight={active ? 600 : 500}
            color={active ? "fg" : "fg.muted"}
            bg={active ? "bg.emphasized" : "transparent"}
            borderLeftWidth={idx === 0 ? 0 : "1px"}
            borderLeftColor="border"
            cursor="pointer"
            _hover={active ? undefined : { color: "fg", bg: "bg.muted" }}
          >
            {isProd && <Dot variant="err" />}
            {env.name}
          </Cell>
        );
      })}
    </HStack>
  );
}
