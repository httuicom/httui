import { Box, Flex, HStack, Text, VStack } from "@chakra-ui/react";
import {
  LuArrowLeftRight,
  LuChevronDown,
  LuChevronRight,
  LuCheck,
  LuDatabase,
  LuFile,
  LuFolder,
  LuFolderOpen,
  LuGitBranch,
  LuKeyRound,
  LuLink,
  LuPlay,
  LuPlus,
  LuSearch,
  LuSettings,
  LuSparkles,
  LuTable,
  LuUsers,
  LuX,
  LuZap,
} from "react-icons/lu";

// WindowChrome — macOS-style traffic lights + title bar.
export function WindowChrome({ title }: { title: string }) {
  return (
    <HStack
      h="30px"
      px={3.5}
      gap={2}
      borderBottom="1px solid"
      borderColor="border"
      bg="bg.surface"
    >
      <Box w="10px" h="10px" rounded="full" bg="#ed6a5e" />
      <Box w="10px" h="10px" rounded="full" bg="#f4be4f" />
      <Box w="10px" h="10px" rounded="full" bg="#62c554" />
      <Text
        flex="1"
        textAlign="center"
        fontFamily="mono"
        fontSize="xs"
        color="fg.muted"
      >
        {title}
      </Text>
      <Box w="40px" />
    </HStack>
  );
}

// MethodPill — colored HTTP method badge.
export function MethodPill({
  method,
}: {
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
}) {
  const tokenMap = {
    GET: "method.get",
    POST: "method.post",
    PUT: "method.put",
    PATCH: "method.patch",
    DELETE: "method.delete",
  } as const;
  return (
    <Text
      as="span"
      fontFamily="mono"
      fontSize="10px"
      fontWeight="600"
      letterSpacing="wider"
      color={tokenMap[method]}
      bg="accent.subtle"
      px={1.5}
      py={0.5}
      rounded="sm"
      _dark={{ bg: "bg.elevated" }}
    >
      {method}
    </Text>
  );
}

// WorkbenchPreview — 1480×940. Top bar + 240/1fr/320 body + status bar.
// Scaled to 0.811 in Hero — only the top portion is visible by design.

// Numbered section heading — accent circle + serif title + rule
function NumberedSection({
  num,
  children,
}: {
  num: number;
  children: React.ReactNode;
}) {
  return (
    <HStack gap={3} alignItems="baseline" mt={6}>
      <Box
        flexShrink={0}
        w="26px"
        h="26px"
        rounded="full"
        bg="accent.subtle"
        color="accent"
        display="grid"
        placeItems="center"
        fontFamily="mono"
        fontSize="12px"
        fontWeight="700"
        border="1px solid"
        borderColor="accent"
      >
        {num}
      </Box>
      <Text
        fontFamily="heading"
        fontSize="22px"
        fontWeight="600"
        color="fg"
        letterSpacing="snug"
        lineHeight="1.25"
      >
        {children}
      </Text>
      <Box flex="1" h="1px" bg="border.subtle" position="relative" top="-4px" />
    </HStack>
  );
}

// BlockShell — bordered card with header (index, optional kind label, body, optional footer)
function BlockShell({
  index,
  kindLabel,
  kindBg,
  pillRight,
  children,
  footer,
}: {
  index: string;
  kindLabel?: string;
  kindBg?: string;
  pillRight?: React.ReactNode;
  children: React.ReactNode;
  footer?: React.ReactNode;
}) {
  return (
    <Box
      mt={3.5}
      border="1px solid"
      borderColor="border"
      rounded="md"
      bg="bg.surface"
      overflow="hidden"
    >
      <HStack
        h="30px"
        px={2}
        gap={2.5}
        bg="bg.elevated"
        borderBottom="1px solid"
        borderColor="border"
      >
        <Text
          w="22px"
          textAlign="center"
          fontFamily="mono"
          fontSize="10px"
          fontWeight="600"
          color="fg.disabled"
        >
          {index}
        </Text>
        {kindLabel && (
          <Text
            fontFamily="mono"
            fontSize="9px"
            fontWeight="700"
            letterSpacing="wider"
            px={1.5}
            py={0.5}
            rounded="sm"
            bg={kindBg ?? "bg.subtle"}
            color="paper.100"
          >
            {kindLabel}
          </Text>
        )}
        <Box flex="1" minW="0">
          {/* When kindLabel is present, the body is below; the children render below the header */}
        </Box>
        <HStack gap={2}>{pillRight}</HStack>
      </HStack>
      <Box px={2.5} py={2}>
        {children}
      </Box>
      {footer}
    </Box>
  );
}

// Section header used across both sidebars
function PaneHead({
  children,
  right,
}: {
  children: React.ReactNode;
  right?: React.ReactNode;
}) {
  return (
    <HStack
      h="28px"
      px="10px"
      gap={2}
      color="fg.subtle"
      fontSize="11px"
      fontWeight="600"
      letterSpacing="wide"
      textTransform="uppercase"
    >
      <Text as="span">{children}</Text>
      <Box flex="1" />
      {right}
    </HStack>
  );
}

function StatusDot({
  kind,
}: {
  kind: "ok" | "warn" | "err" | "info" | "idle";
}) {
  const tokenMap = {
    ok: "ok",
    warn: "warn",
    err: "err",
    info: "info",
    idle: "fg.disabled",
  } as const;
  return (
    <Box
      as="span"
      display="inline-block"
      w="6px"
      h="6px"
      rounded="full"
      bg={tokenMap[kind]}
      flexShrink={0}
    />
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <Box
      as="span"
      display="inline-flex"
      alignItems="center"
      justifyContent="center"
      minW="18px"
      h="18px"
      px="5px"
      fontFamily="mono"
      fontSize="10px"
      fontWeight="500"
      color="fg.muted"
      bg="bg.elevated"
      border="1px solid"
      borderColor="border"
      borderBottomWidth="2px"
      rounded="sm"
    >
      {children}
    </Box>
  );
}

export function WorkbenchPreview() {
  return (
    <Box
      w="1480px"
      h="940px"
      bg="bg"
      color="fg"
      display="grid"
      gridTemplateColumns="240px 1fr 320px"
      gridTemplateRows="36px 1fr 22px"
      fontFamily="body"
      fontSize="13px"
      border="1px solid"
      borderColor="border"
      overflow="hidden"
    >
      {/* TOP BAR — spans 3 columns */}
      <HStack
        gridColumn="1 / 4"
        gridRow="1"
        h="36px"
        px="10px"
        gap={3}
        bg="bg.surface"
        borderBottom="1px solid"
        borderColor="border"
      >
        {/* Logo */}
        <HStack gap={2} color="accent" fontWeight="700" letterSpacing="snug">
          <Box w="14px" h="14px" rounded="sm" bg="accent" />
          <Text>httui</Text>
        </HStack>
        <Box w="1px" h="18px" bg="border" />

        {/* Breadcrumb */}
        <HStack gap={1.5} fontSize="12px" color="fg.subtle">
          <Text>acme</Text>
          <LuChevronRight size={12} />
          <Text>payments</Text>
          <LuChevronRight size={12} />
          <Text color="fg">rollout-v2.3.md</Text>
          <StatusDot kind="warn" />
        </HStack>
        <Box flex="1" />

        {/* Env switcher */}
        <HStack
          gap={0}
          h="24px"
          border="1px solid"
          borderColor="border"
          rounded="sm"
          overflow="hidden"
        >
          {(["local", "staging", "prod"] as const).map((e, i) => {
            const active = e === "staging";
            return (
              <HStack
                key={e}
                px={2.5}
                h="100%"
                gap={1.5}
                fontSize="12px"
                fontWeight={active ? "600" : "400"}
                bg={active ? "bg.subtle" : "transparent"}
                color={active ? "fg" : "fg.subtle"}
                borderRight={i < 2 ? "1px solid" : "none"}
                borderColor="border"
              >
                {e === "prod" && <StatusDot kind="err" />}
                <Text>{e}</Text>
              </HStack>
            );
          })}
        </HStack>

        {/* Search */}
        <HStack
          h="24px"
          px={2}
          gap={1.5}
          w="220px"
          bg="bg.elevated"
          border="1px solid"
          borderColor="border"
          rounded="sm"
          color="fg.disabled"
        >
          <LuSearch size={12} />
          <Text fontSize="12px" flex="1" truncate>
            Search blocks, vars, schema…
          </Text>
          <Kbd>⌘K</Kbd>
        </HStack>

        {/* Collaborators */}
        <HStack gap={0}>
          {[
            { id: "rf", n: "R", c: "oklch(0.78 0.16 145)" },
            { id: "ml", n: "M", c: "oklch(0.74 0.18 30)" },
            { id: "td", n: "T", c: "oklch(0.74 0.14 250)" },
          ].map((c, i) => (
            <Box
              key={c.id}
              w="22px"
              h="22px"
              rounded="full"
              bg={c.c}
              color="oklch(0.18 0.02 260)"
              fontSize="10px"
              fontWeight="700"
              display="grid"
              placeItems="center"
              ml={i === 0 ? 0 : "-6px"}
              border="2px solid"
              borderColor="bg.surface"
            >
              {c.n}
            </Box>
          ))}
        </HStack>

        {/* Branch + Run all */}
        <HStack
          h="24px"
          px={2}
          gap={1.5}
          color="fg.muted"
          fontSize="12px"
          bg="transparent"
          rounded="sm"
        >
          <LuGitBranch size={12} />
          <Text>main</Text>
        </HStack>
        <HStack
          h="24px"
          px={2.5}
          gap={1.5}
          bg="accent"
          color="accent.fg"
          fontSize="11px"
          fontWeight="600"
          rounded="sm"
        >
          <LuPlay size={11} fill="currentColor" />
          <Text>Run all</Text>
        </HStack>
      </HStack>

      {/* LEFT SIDEBAR — Files + Connections + Variables */}
      <Flex
        gridColumn="1"
        gridRow="2"
        direction="column"
        bg="bg.surface"
        borderRight="1px solid"
        borderColor="border"
        minH="0"
      >
        {/* Files */}
        <PaneHead right={<LuPlus size={11} />}>Files</PaneHead>
        <Box
          flex="1 1 50%"
          overflow="hidden"
          borderBottom="1px solid"
          borderColor="border"
          pb={1.5}
        >
          {(
            [
              { n: "runbooks", folder: true, depth: 0, open: true },
              { n: "payments", folder: true, depth: 1, open: true },
              {
                n: "rollout-v2.3.md",
                folder: false,
                depth: 2,
                active: true,
                dirty: true,
              },
              { n: "rollback.md", folder: false, depth: 2 },
              { n: "incident-2026-03-19.md", folder: false, depth: 2 },
              { n: "onboarding", folder: true, depth: 1, open: false },
              { n: "data-fixes.md", folder: false, depth: 1 },
              { n: "scratch", folder: true, depth: 0, open: false },
              { n: "ad-hoc.md", folder: false, depth: 1 },
              { n: "shared / team", folder: true, depth: 0, open: false },
            ] as Array<{
              n: string;
              folder: boolean;
              depth: number;
              open?: boolean;
              active?: boolean;
              dirty?: boolean;
            }>
          ).map((r, i) => (
            <HStack
              key={i}
              h="24px"
              pl={`${8 + r.depth * 12}px`}
              pr={2}
              gap={1.5}
              fontSize="12px"
              color={r.active ? "fg" : "fg.muted"}
              fontWeight={r.active ? "600" : "400"}
              bg={r.active ? "bg.subtle" : "transparent"}
            >
              <Box
                w="10px"
                color="fg.disabled"
                display="flex"
                alignItems="center"
              >
                {r.folder ? (
                  r.open ? (
                    <LuChevronDown size={11} />
                  ) : (
                    <LuChevronRight size={11} />
                  )
                ) : null}
              </Box>
              <Box
                color={r.folder ? "fg.subtle" : "accent"}
                display="flex"
                alignItems="center"
              >
                {r.folder ? (
                  r.open ? (
                    <LuFolderOpen size={12} />
                  ) : (
                    <LuFolder size={12} />
                  )
                ) : (
                  <LuFile size={12} />
                )}
              </Box>
              <Text flex="1" truncate>
                {r.n}
              </Text>
              {r.dirty && <StatusDot kind="warn" />}
            </HStack>
          ))}
        </Box>

        {/* Connections */}
        <Box borderBottom="1px solid" borderColor="border">
          <PaneHead right={<LuPlus size={11} />}>Connections</PaneHead>
          <VStack align="stretch" gap={0} pb={1.5}>
            {[
              { n: "pg · payments@staging", k: "ok", l: 18 },
              { n: "pg · payments@prod", k: "ok", l: 41, prod: true },
              { n: "mongo · audit", k: "ok", l: 22 },
              { n: "redis · cache", k: "warn", l: 88 },
              { n: "API · payments", k: "ok", l: 142 },
            ].map((c, i) => (
              <HStack
                key={i}
                h="26px"
                px={2.5}
                gap={2}
                fontSize="12px"
                color="fg.muted"
              >
                <StatusDot kind={c.k as "ok" | "warn"} />
                <Box color="fg.subtle" display="flex" alignItems="center">
                  <LuDatabase size={12} />
                </Box>
                <Text flex="1" truncate>
                  {c.n}
                </Text>
                {c.prod && (
                  <Text
                    fontSize="9px"
                    fontWeight="700"
                    color="err"
                    letterSpacing="wide"
                  >
                    PROD
                  </Text>
                )}
                <Text fontFamily="mono" fontSize="10px" color="fg.disabled">
                  {c.l}ms
                </Text>
              </HStack>
            ))}
          </VStack>
        </Box>

        {/* Variables — staging */}
        <Box>
          <PaneHead right={<LuPlus size={11} />}>Variables — staging</PaneHead>
          <VStack align="stretch" gap={0} pb={2}>
            {[
              {
                k: "BASE_URL",
                v: "https://api.staging.acme.dev",
                secret: false,
              },
              { k: "TENANT_ID", v: "tnt_8f2a91", secret: false },
              { k: "ADMIN_TOKEN", v: "••••••••••••mB9k", secret: true },
              { k: "PG_DSN", v: "postgres://app@db-staging…", secret: false },
            ].map((v) => (
              <HStack key={v.k} h="22px" px={2.5} gap={1.5} fontSize="11px">
                <Box
                  color={v.secret ? "warn" : "fg.disabled"}
                  display="flex"
                  alignItems="center"
                  w="11px"
                  justifyContent="center"
                >
                  {v.secret ? (
                    <LuKeyRound size={11} />
                  ) : (
                    <Box w="3px" h="3px" rounded="full" bg="fg.disabled" />
                  )}
                </Box>
                <Text fontFamily="mono" color="fg">
                  {v.k}
                </Text>
                <Text
                  fontFamily="mono"
                  color="fg.subtle"
                  flex="1"
                  truncate
                  textAlign="right"
                  maxW="130px"
                >
                  {v.v}
                </Text>
              </HStack>
            ))}
          </VStack>
        </Box>
      </Flex>

      {/* CENTER — tabs + toolbar + DocHeader + section 1 */}
      <Flex gridColumn="2" gridRow="2" direction="column" minH="0" bg="bg">
        {/* Tab bar */}
        <HStack
          h="32px"
          gap={0}
          bg="bg.surface"
          borderBottom="1px solid"
          borderColor="border"
        >
          <HStack
            h="100%"
            px={3.5}
            gap={2}
            bg="bg"
            borderRight="1px solid"
            borderColor="border"
            position="relative"
            _after={{
              content: '""',
              position: "absolute",
              top: 0,
              left: 0,
              right: 0,
              h: "1px",
              bg: "accent",
            }}
          >
            <Box color="accent" display="flex" alignItems="center">
              <LuFile size={12} />
            </Box>
            <Text fontSize="12px" color="fg">
              rollout-v2.3.md
            </Text>
            <StatusDot kind="warn" />
            <Box color="fg.disabled" display="flex" alignItems="center" ml={1}>
              <LuX size={12} />
            </Box>
          </HStack>
          <HStack
            h="100%"
            px={3.5}
            gap={2}
            borderRight="1px solid"
            borderColor="border"
          >
            <Box color="fg.disabled" display="flex" alignItems="center">
              <LuFile size={12} />
            </Box>
            <Text fontSize="12px" color="fg.subtle">
              rollback.md
            </Text>
            <Box color="fg.disabled" display="flex" alignItems="center">
              <LuX size={12} />
            </Box>
          </HStack>
          <HStack
            h="100%"
            px={3.5}
            gap={2}
            borderRight="1px solid"
            borderColor="border"
          >
            <Box color="fg.disabled" display="flex" alignItems="center">
              <LuFile size={12} />
            </Box>
            <Text fontSize="12px" color="fg.subtle">
              ad-hoc.md
            </Text>
            <Box color="fg.disabled" display="flex" alignItems="center">
              <LuX size={12} />
            </Box>
          </HStack>
          <Box flex="1" borderRight="1px solid" borderColor="border" />
          <HStack px={2.5} gap={2.5} color="fg.subtle">
            <LuArrowLeftRight size={14} />
            <LuUsers size={14} />
            <LuSettings size={14} />
          </HStack>
        </HStack>

        {/* Editor toolbar */}
        <HStack
          h="30px"
          px={3}
          gap={3}
          bg="bg.surface"
          borderBottom="1px solid"
          borderColor="border"
          fontSize="11px"
          color="fg.subtle"
        >
          <Text>runbooks / payments / rollout-v2.3.md</Text>
          <Text>·</Text>
          <Text>edited há 4 min by rafael</Text>
          <Box flex="1" />
          <Text fontFamily="mono">10 blocks · 4 ran · 1 pending</Text>
          <Text>·</Text>
          <HStack gap={1} color="accent">
            <LuZap size={11} />
            <Text>auto-capture</Text>
          </HStack>
        </HStack>

        {/* Document */}
        <Box flex="1" overflow="hidden" px="36px" pt="20px" pb="60px">
          <Box maxW="880px" mx="auto">
            {/* DocHeader frontmatter */}
            <Box pb={5} borderBottom="1px solid" borderColor="border">
              {/* breadcrumb */}
              <HStack
                gap={1.5}
                fontSize="11px"
                color="fg.disabled"
                fontFamily="mono"
                mb={3.5}
              >
                <LuFolder size={11} />
                <Text>runbooks</Text>
                <Text>/</Text>
                <Text>payments</Text>
                <Text>/</Text>
                <Text color="fg.muted">rollout-v2.3.md</Text>
                <Box flex="1" />
                <HStack
                  gap={1.5}
                  px="7px"
                  py="2px"
                  border="1px solid"
                  borderColor="border"
                  rounded="sm"
                  fontSize="10px"
                  color="fg.subtle"
                >
                  <StatusDot kind="warn" />
                  <Text>draft · 4 unsaved edits</Text>
                </HStack>
              </HStack>

              {/* Title */}
              <Text
                as="h1"
                fontFamily="heading"
                fontSize="32px"
                lineHeight="1.18"
                fontWeight="700"
                letterSpacing="snug"
                color="fg"
                mb={2}
              >
                Rollout — Payments v2.3 → staging
              </Text>

              {/* Meta strip */}
              <HStack
                gap={3.5}
                flexWrap="wrap"
                mb={4}
                fontSize="12px"
                color="fg.subtle"
              >
                <HStack gap={1.5}>
                  <Box
                    w="18px"
                    h="18px"
                    rounded="full"
                    bg="oklch(0.74 0.14 50)"
                    color="white"
                    fontSize="10px"
                    fontWeight="700"
                    display="grid"
                    placeItems="center"
                  >
                    R
                  </Box>
                  <Text color="fg.muted">rafael</Text>
                </HStack>
                <Text>·</Text>
                <Text>
                  edited{" "}
                  <Text as="span" fontFamily="mono" color="fg.muted">
                    há 4 min
                  </Text>
                </Text>
                <Text>·</Text>
                <Text fontFamily="mono">10 blocks</Text>
                <Text>·</Text>
                <HStack gap={1.5}>
                  <LuGitBranch size={11} />
                  <Text fontFamily="mono" color="fg.muted">
                    main
                  </Text>
                  <Text color="accent.emphasized" fontWeight="600">
                    +3 ~1
                  </Text>
                </HStack>
                <Text>·</Text>
                <HStack gap={1.5} color="ok">
                  <StatusDot kind="ok" />
                  <Text>last run 14:23 — all green</Text>
                </HStack>
              </HStack>

              {/* Abstract */}
              <Text
                fontFamily="heading"
                fontSize="16px"
                lineHeight="1.6"
                color="fg.muted"
                mb={3.5}
                maxW="720px"
              >
                Deploy do novo provider de cartão (
                <Text
                  as="code"
                  fontFamily="mono"
                  px={1}
                  bg="bg.elevated"
                  rounded="sm"
                  fontSize="13px"
                >
                  stripe_v2
                </Text>
                ) para o tenant{" "}
                <Text
                  as="code"
                  fontFamily="mono"
                  px={1}
                  bg="bg.elevated"
                  rounded="sm"
                  fontSize="13px"
                >
                  acme-payments
                </Text>{" "}
                em staging. Antes de promover para prod, validar que a config
                foi propagada, que{" "}
                <Text
                  as="code"
                  fontFamily="mono"
                  px={1}
                  bg="bg.elevated"
                  rounded="sm"
                  fontSize="13px"
                >
                  payments_route
                </Text>{" "}
                não tem rotas órfãs, e que a latência do stream de captura fica
                abaixo de 800ms.
              </Text>

              {/* Pre-flight + Tags */}
              <Box
                display="grid"
                gridTemplateColumns="1fr auto"
                gap={6}
                alignItems="start"
              >
                <Box
                  bg="bg.surface"
                  border="1px solid"
                  borderColor="border.subtle"
                  rounded="md"
                  px={3.5}
                  py={2.5}
                >
                  <Text
                    fontSize="10px"
                    fontWeight="700"
                    letterSpacing="wider"
                    color="fg.subtle"
                    mb={1.5}
                  >
                    PRÉ-FLIGHT — 4 itens
                  </Text>
                  <VStack align="stretch" gap={1} fontSize="13px">
                    {[
                      {
                        ok: true,
                        t: "config do tenant atualizada em ",
                        code: "tenants.config",
                      },
                      {
                        ok: true,
                        t: "0 registros órfãos em ",
                        code: "payments_route",
                      },
                      {
                        ok: false,
                        t: "subscrever stream WS por ",
                        code: "30s",
                      },
                      { ok: false, t: "rollout_pct ≤ 25 antes de prod" },
                    ].map((it, i) => (
                      <HStack
                        key={i}
                        gap={2}
                        color={it.ok ? "fg.subtle" : "fg"}
                        align="center"
                      >
                        <Box
                          w="14px"
                          h="14px"
                          rounded="xs"
                          border={it.ok ? "none" : "1px solid"}
                          borderColor="border"
                          bg={it.ok ? "ok" : "transparent"}
                          color="white"
                          display="grid"
                          placeItems="center"
                          flexShrink={0}
                        >
                          {it.ok && <LuCheck size={10} strokeWidth={3} />}
                        </Box>
                        <Text
                          textDecoration={it.ok ? "line-through" : "none"}
                          textDecorationColor="var(--chakra-colors-fg-disabled)"
                        >
                          {it.t}
                          {it.code && (
                            <Text
                              as="code"
                              fontFamily="mono"
                              px={1}
                              bg="bg.elevated"
                              rounded="sm"
                              fontSize="12px"
                            >
                              {it.code}
                            </Text>
                          )}
                        </Text>
                      </HStack>
                    ))}
                  </VStack>
                </Box>
                <VStack align="stretch" gap={1.5} fontSize="11px">
                  <Text
                    fontSize="10px"
                    fontWeight="700"
                    letterSpacing="wider"
                    color="fg.disabled"
                  >
                    TAGS
                  </Text>
                  {[
                    "#rollout",
                    "#payments",
                    "#staging",
                    "#breaking-change",
                  ].map((t) => (
                    <Text key={t} fontFamily="mono" color="fg.muted">
                      {t}
                    </Text>
                  ))}
                </VStack>
              </Box>
            </Box>

            {/* Section 1 */}
            <NumberedSection num={1}>
              Sanidade — health check do gateway
            </NumberedSection>

            {/* Block 02 — HTTP GET /v2/health */}
            <BlockShell
              index="02"
              pillRight={
                <>
                  <Text fontSize="11px" color="fg.disabled">
                    ran 14:22:08
                  </Text>
                  <HStack
                    gap={1.5}
                    fontFamily="mono"
                    fontSize="11px"
                    color="ok"
                    fontWeight="600"
                  >
                    <StatusDot kind="ok" />
                    <Text>200 · 142ms</Text>
                  </HStack>
                </>
              }
            >
              <HStack gap={2}>
                <MethodPill method="GET" />
                <Text
                  fontFamily="mono"
                  fontSize="12px"
                  color="fg"
                  flex="1"
                  truncate
                >
                  {"{{BASE_URL}}/v2/health"}
                </Text>
              </HStack>
              <Box
                mt={1.5}
                fontFamily="mono"
                fontSize="11px"
                lineHeight="1.6"
                color="fg.muted"
              >
                <Text>{"{"}</Text>
                <Text pl={3.5}>
                  <Text as="span" color="accent.emphasized">
                    "status"
                  </Text>
                  :{" "}
                  <Text as="span" color="ok">
                    "ok"
                  </Text>
                  ,{" "}
                  <Text as="span" color="accent.emphasized">
                    "version"
                  </Text>
                  :{" "}
                  <Text as="span" color="ok">
                    "2.3.0-rc4"
                  </Text>
                  ,
                </Text>
                <Text pl={3.5}>
                  <Text as="span" color="accent.emphasized">
                    "deps"
                  </Text>
                  :{" "}
                  <Text as="span" color="fg.muted">
                    {"{ "}
                  </Text>
                  <Text as="span" color="accent.emphasized">
                    "db"
                  </Text>
                  :{" "}
                  <Text as="span" color="ok">
                    "ok"
                  </Text>
                  ,{" "}
                  <Text as="span" color="accent.emphasized">
                    "kafka"
                  </Text>
                  :{" "}
                  <Text as="span" color="warn">
                    "degraded"
                  </Text>
                  <Text as="span" color="fg.muted">
                    {" }"}
                  </Text>
                </Text>
                <Text>{"}"}</Text>
              </Box>
            </BlockShell>

            {/* Section 2 */}
            <NumberedSection num={2}>
              Autenticar como admin do tenant
            </NumberedSection>

            {/* Block 04 — POST /v2/auth/admin */}
            <BlockShell
              index="04"
              pillRight={
                <>
                  <Text fontSize="11px" color="fg.disabled">
                    ran 14:22:14
                  </Text>
                  <HStack
                    gap={1.5}
                    fontFamily="mono"
                    fontSize="11px"
                    color="ok"
                    fontWeight="600"
                  >
                    <StatusDot kind="ok" />
                    <Text>201 · 318ms</Text>
                  </HStack>
                </>
              }
              footer={
                <HStack
                  px={2.5}
                  py={1.5}
                  gap={1.5}
                  bg="accent.subtle"
                  borderTop="1px solid"
                  borderColor="border.subtle"
                  fontSize="10px"
                  color="fg.muted"
                >
                  <LuLink size={11} color="var(--chakra-colors-accent)" />
                  <Text>captured →</Text>
                  <Text
                    fontFamily="mono"
                    color="accent.emphasized"
                    fontWeight="600"
                  >
                    SESSION_ID
                  </Text>
                  <Text fontFamily="mono">= ses_01HZ4RTQ8VK7…</Text>
                </HStack>
              }
            >
              <HStack gap={2}>
                <MethodPill method="POST" />
                <Text
                  fontFamily="mono"
                  fontSize="12px"
                  color="fg"
                  flex="1"
                  truncate
                >
                  {"{{BASE_URL}}/v2/auth/admin"}
                </Text>
              </HStack>
              <Box
                mt={1.5}
                fontFamily="mono"
                fontSize="11px"
                lineHeight="1.6"
                color="fg.muted"
              >
                <Text>
                  <Text as="span" color="fg.subtle">
                    Authorization
                  </Text>
                  : Bearer{" "}
                  <Text as="span" color="accent.emphasized">
                    {"{{ADMIN_TOKEN}}"}
                  </Text>
                </Text>
                <Text>
                  <Text as="span" color="fg.subtle">
                    X-Tenant
                  </Text>
                  :{" "}
                  <Text as="span" color="accent.emphasized">
                    {"{{TENANT_ID}}"}
                  </Text>
                </Text>
              </Box>
            </BlockShell>

            {/* Section 3 */}
            <NumberedSection num={3}>
              Verificar rotas órfãs no banco
            </NumberedSection>

            {/* Block 06 — SQL */}
            <BlockShell
              index="06"
              kindLabel="SQL"
              kindBg="oklch(0.42 0.10 290)"
              pillRight={
                <>
                  <Text fontSize="11px" color="fg.disabled">
                    pg · payments@staging
                  </Text>
                  <HStack
                    gap={1.5}
                    fontFamily="mono"
                    fontSize="11px"
                    color="ok"
                    fontWeight="600"
                  >
                    <StatusDot kind="ok" />
                    <Text>3 rows · 47ms</Text>
                  </HStack>
                </>
              }
            >
              <Box
                fontFamily="mono"
                fontSize="11px"
                lineHeight="1.7"
                color="fg.muted"
              >
                <Text>
                  <Text as="span" color="info">
                    SELECT
                  </Text>{" "}
                  r.id, r.tenant_id, r.provider_key
                </Text>
                <Text>
                  <Text as="span" color="info">
                    FROM
                  </Text>{" "}
                  payments_route r
                </Text>
                <Text>
                  <Text as="span" color="info">
                    LEFT JOIN
                  </Text>{" "}
                  payment_provider p{" "}
                  <Text as="span" color="info">
                    ON
                  </Text>{" "}
                  p.key = r.provider_key
                </Text>
                <Text>
                  <Text as="span" color="info">
                    WHERE
                  </Text>{" "}
                  p.key{" "}
                  <Text as="span" color="info">
                    IS NULL
                  </Text>
                  ;
                </Text>
              </Box>
            </BlockShell>
          </Box>
        </Box>
      </Flex>

      {/* RIGHT SIDEBAR — Outline / Schema / History / Comments */}
      <Flex
        gridColumn="3"
        gridRow="2"
        direction="column"
        bg="bg.surface"
        borderLeft="1px solid"
        borderColor="border"
        minH="0"
      >
        {/* Tabs */}
        <HStack
          h="30px"
          gap={0}
          bg="bg"
          borderBottom="1px solid"
          borderColor="border"
        >
          {(["Outline", "Schema", "History", "Comments"] as const).map((t) => {
            const active = t === "Schema";
            const last = t === "Comments";
            return (
              <HStack
                key={t}
                flex="1"
                h="100%"
                justify="center"
                gap={1.5}
                fontSize="11px"
                fontWeight={active ? "600" : "500"}
                color={active ? "fg" : "fg.subtle"}
                bg={active ? "bg.surface" : "transparent"}
                borderBottom={active ? "1px solid" : "1px solid transparent"}
                borderBottomColor={active ? "accent" : "transparent"}
                borderRight={!last ? "1px solid" : "none"}
                borderRightColor="border"
              >
                <Text>{t}</Text>
                {t === "Comments" && (
                  <Text
                    fontSize="9px"
                    px="5px"
                    py="1px"
                    bg="accent"
                    color="accent.fg"
                    rounded="full"
                    fontWeight="700"
                  >
                    2
                  </Text>
                )}
              </HStack>
            );
          })}
        </HStack>

        {/* Schema panel */}
        <Box flex="1" overflow="hidden">
          <PaneHead
            right={
              <Text fontSize="10px" color="fg.disabled">
                payments
              </Text>
            }
          >
            Database
          </PaneHead>
          <HStack px={3} pb={1.5} gap={1.5} fontSize="11px" color="fg.subtle">
            <StatusDot kind="ok" />
            <Text>pg · payments@staging · 18ms</Text>
          </HStack>

          {/* public schema */}
          <HStack h="22px" px={3} gap={1.5} fontSize="12px" color="fg.muted">
            <Box color="fg.disabled" display="flex" alignItems="center">
              <LuChevronDown size={11} />
            </Box>
            <Box color="fg.subtle" display="flex" alignItems="center">
              <LuDatabase size={12} />
            </Box>
            <Text>public</Text>
            <Box flex="1" />
            <Text fontSize="10px" color="fg.disabled">
              5
            </Text>
          </HStack>

          {[
            { n: "payment_provider", rows: "12", open: false },
            { n: "payments_route", rows: "4,318", open: true },
            { n: "tenants", rows: "821", open: false },
            { n: "captures", rows: "1,284,913", open: false },
            { n: "audit_log", rows: "9,123,402", open: false },
          ].map((t) => (
            <Box key={t.n}>
              <HStack
                h="22px"
                pl="26px"
                pr={3}
                gap={1.5}
                fontSize="12px"
                color="fg"
                bg={t.open ? "bg.elevated" : "transparent"}
                fontWeight={t.open ? "600" : "400"}
              >
                <Box color="fg.disabled" display="flex" alignItems="center">
                  {t.open ? (
                    <LuChevronDown size={11} />
                  ) : (
                    <LuChevronRight size={11} />
                  )}
                </Box>
                <Box color="fg.subtle" display="flex" alignItems="center">
                  <LuTable size={11} />
                </Box>
                <Text fontFamily="mono">{t.n}</Text>
                <Box flex="1" />
                <Text fontFamily="mono" fontSize="10px" color="fg.disabled">
                  {t.rows}
                </Text>
              </HStack>
              {t.open && (
                <>
                  {[
                    { k: "id", t: "text", pk: true },
                    { k: "tenant_id", t: "text", fk: true },
                    { k: "provider_key", t: "text", fk: true },
                    { k: "fallbacks", t: "text[]" },
                    { k: "rollout_pct", t: "int4" },
                    { k: "created_at", t: "timestamptz" },
                    { k: "updated_at", t: "timestamptz" },
                  ].map((c) => (
                    <HStack
                      key={c.k}
                      h="20px"
                      pl="50px"
                      pr={3}
                      gap={2}
                      fontSize="11px"
                      color="fg.muted"
                    >
                      <Box
                        w="11px"
                        display="flex"
                        alignItems="center"
                        color={c.pk ? "warn" : c.fk ? "info" : "fg.disabled"}
                      >
                        {c.pk ? (
                          <LuKeyRound size={10} />
                        ) : c.fk ? (
                          <LuLink size={10} />
                        ) : (
                          <Box
                            w="3px"
                            h="3px"
                            rounded="full"
                            bg="fg.disabled"
                          />
                        )}
                      </Box>
                      <Text fontFamily="mono" color="fg">
                        {c.k}
                      </Text>
                      <Box flex="1" />
                      <Text fontFamily="mono" color="fg.disabled">
                        {c.t}
                      </Text>
                    </HStack>
                  ))}
                </>
              )}
            </Box>
          ))}

          {/* billing schema */}
          <HStack
            h="22px"
            px={3}
            gap={1.5}
            mt={1.5}
            fontSize="12px"
            color="fg.muted"
          >
            <Box color="fg.disabled" display="flex" alignItems="center">
              <LuChevronRight size={11} />
            </Box>
            <Box color="fg.subtle" display="flex" alignItems="center">
              <LuDatabase size={12} />
            </Box>
            <Text>billing</Text>
            <Box flex="1" />
            <Text fontSize="10px" color="fg.disabled">
              3
            </Text>
          </HStack>

          {/* History (cropped) */}
          <Box h="1px" bg="border" my={3} />
          <PaneHead
            right={
              <Text fontSize="10px" color="fg.disabled">
                6
              </Text>
            }
          >
            History
          </PaneHead>
          {[
            {
              at: "14:24:07",
              label: "WS captures",
              info: "14 msgs",
              k: "info" as const,
            },
            {
              at: "14:23:01",
              label: "SELECT … rotas órfãs",
              info: "3 rows · 47ms",
              k: "ok" as const,
            },
            {
              at: "14:22:14",
              label: "POST /v2/auth/admin",
              info: "201 · 318ms",
              k: "ok" as const,
            },
            {
              at: "14:22:08",
              label: "GET /v2/health",
              info: "200 · 142ms",
              k: "ok" as const,
            },
          ].map((h, i) => (
            <HStack key={i} h="24px" px={3} gap={2} fontSize="11px">
              <Text fontFamily="mono" color="fg.disabled" w="44px">
                {h.at}
              </Text>
              <Text color="fg" flex="1" truncate>
                {h.label}
              </Text>
              <Text
                fontFamily="mono"
                fontSize="10px"
                fontWeight="600"
                color={h.k === "ok" ? "ok" : "info"}
              >
                {h.info}
              </Text>
            </HStack>
          ))}
        </Box>

        {/* AI bar */}
        <Box
          borderTop="1px solid"
          borderColor="border"
          px={2.5}
          py={2}
          bg="bg.elevated"
        >
          <HStack mb={1.5} fontSize="11px" color="fg.subtle" gap={1.5}>
            <Box color="accent" display="flex" alignItems="center">
              <LuSparkles size={12} />
            </Box>
            <Text color="fg" fontWeight="600">
              Ask httui
            </Text>
            <Box flex="1" />
            <Kbd>⌘J</Kbd>
          </HStack>
          <HStack
            h="26px"
            px={2}
            bg="bg"
            border="1px solid"
            borderColor="border"
            rounded="sm"
            fontSize="12px"
            color="fg.disabled"
          >
            <Text>Por que kafka está degraded?</Text>
          </HStack>
        </Box>
      </Flex>

      {/* STATUS BAR — spans 3 columns */}
      <HStack
        gridColumn="1 / 4"
        gridRow="3"
        h="22px"
        px={2.5}
        gap={3.5}
        fontFamily="mono"
        fontSize="11px"
        color="fg.subtle"
        bg="bg.surface"
        borderTop="1px solid"
        borderColor="border"
      >
        <HStack gap={1.5}>
          <LuGitBranch size={11} />
          <Text>main · 3 changes</Text>
        </HStack>
        <Box w="1px" h="12px" bg="border" />
        <HStack gap={1.5}>
          <StatusDot kind="ok" />
          <Text>connected · staging</Text>
        </HStack>
        <Box w="1px" h="12px" bg="border" />
        <Text>pg 18ms</Text>
        <Text>api 142ms</Text>
        <Box w="1px" h="12px" bg="border" />
        <Text>Ln 47, Col 12</Text>
        <Box flex="1" />
        <Text>UTF-8</Text>
        <Box w="1px" h="12px" bg="border" />
        <HStack gap={1} color="accent">
          <LuZap size={10} />
          <Text>chained</Text>
        </HStack>
        <Box w="1px" h="12px" bg="border" />
        <Text>httui 0.4.2</Text>
      </HStack>
    </Box>
  );
}

// BlocksPreview — feature 1: markdown + HTTP + SQL chained.
export function BlocksPreview() {
  return (
    <Box
      bg="bg.surface"
      border="1px solid"
      borderColor="border"
      rounded="lg"
      p={4}
      fontFamily="mono"
      fontSize="xs"
      shadow="card"
    >
      <Text
        fontFamily="heading"
        fontSize="lg"
        fontWeight="600"
        color="fg"
        mb={1}
      >
        1. Verify shadow traffic
      </Text>
      <Text
        fontFamily="heading"
        fontSize="13px"
        lineHeight="1.55"
        color="fg.muted"
        mb={3}
      >
        Confirm{" "}
        <Text
          as="code"
          fontFamily="mono"
          fontSize="12px"
          px={1}
          bg="bg.elevated"
          rounded="sm"
        >
          payments-router
        </Text>{" "}
        is mirroring 5% of requests in staging.
      </Text>

      {/* HTTP block */}
      <Box
        bg="bg.elevated"
        border="1px solid"
        borderColor="border"
        rounded="md"
        mb={2.5}
        overflow="hidden"
      >
        <HStack
          h="28px"
          px={2.5}
          bg="bg.subtle"
          borderBottom="1px solid"
          borderColor="border"
          gap={2}
        >
          <MethodPill method="POST" />
          <Text fontSize="xs" color="fg">
            {"{{api}}/v2/payments"}
          </Text>
          <Box flex="1" />
          <Text fontSize="11px" color="ok" fontWeight="600">
            ● 201 · 218ms
          </Text>
        </HStack>
        <Box px={2.5} py={2} fontSize="11px" lineHeight="1.6" color="fg.muted">
          <Text>{"{"}</Text>
          <Text pl={3.5}>
            <Text as="span" color="accent.emphasized">
              "id"
            </Text>
            :{" "}
            <Text as="span" color="ok">
              "pay_01H8XK..."
            </Text>
            ,
          </Text>
          <Text pl={3.5}>
            <Text as="span" color="accent.emphasized">
              "provider"
            </Text>
            :{" "}
            <Text as="span" color="ok">
              "stripe_v2"
            </Text>
            ,
          </Text>
          <Text pl={3.5}>
            <Text as="span" color="accent.emphasized">
              "shadow"
            </Text>
            :{" "}
            <Text as="span" color="info">
              true
            </Text>
          </Text>
          <Text>{"}"}</Text>
        </Box>
        <HStack
          px={2.5}
          py={1.5}
          bg="accent.subtle"
          borderTop="1px solid"
          borderColor="border.subtle"
          fontSize="10px"
          color="fg.muted"
          gap={1.5}
        >
          <LuLink size={11} color="var(--chakra-colors-accent)" />
          <Text>captured:</Text>
          <Text fontWeight="600" color="accent.emphasized">
            payment_id
          </Text>
          <Text>= pay_01H8XK…</Text>
        </HStack>
      </Box>

      {/* SQL block */}
      <Box
        bg="bg.elevated"
        border="1px solid"
        borderColor="border"
        rounded="md"
        overflow="hidden"
      >
        <HStack
          h="28px"
          px={2.5}
          bg="bg.subtle"
          borderBottom="1px solid"
          borderColor="border"
          gap={2}
        >
          <Text
            fontFamily="mono"
            fontSize="9px"
            fontWeight="700"
            px={1.5}
            py={0.5}
            bg="moss"
            color="paper.100"
            rounded="sm"
          >
            SQL
          </Text>
          <Text fontSize="xs" color="fg">
            pg · payments@staging
          </Text>
          <Box flex="1" />
          <Text fontSize="11px" color="ok" fontWeight="600">
            ● 1 row · 14ms
          </Text>
        </HStack>
        <Box px={2.5} py={2} fontSize="11px" lineHeight="1.7" color="fg.muted">
          <Text>
            <Text as="span" color="info">
              SELECT
            </Text>{" "}
            status, provider{" "}
            <Text as="span" color="info">
              FROM
            </Text>{" "}
            payments
          </Text>
          <Text>
            <Text as="span" color="info">
              WHERE
            </Text>{" "}
            id ={" "}
            <Text as="span" color="accent.emphasized">
              {"{{payment_id}}"}
            </Text>
          </Text>
        </Box>
      </Box>
    </Box>
  );
}

// SchemaPreview — feature 2: schema explorer + result.
type TreeNode = {
  n: string;
  indent: number;
  bold?: boolean;
  active?: boolean;
  count?: string;
  key?: boolean;
  link?: boolean;
};

export function SchemaPreview() {
  const tree: TreeNode[] = [
    { n: "▾ public", indent: 0, bold: true, count: "4 tables" },
    { n: "▸ payments", indent: 1, count: "84.2k" },
    { n: "▾ payments_route", indent: 1, count: "4.3k", active: true },
    { n: "id  uuid", indent: 2, key: true },
    { n: "provider_key  text", indent: 2, link: true },
    { n: "tenant_id  uuid", indent: 2, link: true },
    { n: "active  bool", indent: 2 },
    { n: "created_at  timestamptz", indent: 2 },
    { n: "▸ payment_provider", indent: 1, count: "12" },
    { n: "▸ tenants", indent: 1, count: "284" },
    { n: "▾ analytics", indent: 0, bold: true, count: "1 table" },
    { n: "▸ events", indent: 1, count: "12.4M" },
  ];
  const rows = [
    ["1", "rt_4f2a", "stripe_v2", "acme", "true", "2026-04-24 14:08"],
    ["2", "rt_7ce1", "stripe_v2", "acme", "true", "2026-04-24 13:51"],
    ["3", "rt_b08d", "adyen_v1", "acme", "false", "2026-04-23 09:12"],
  ];
  return (
    <Box
      bg="bg.surface"
      border="1px solid"
      borderColor="border"
      rounded="lg"
      overflow="hidden"
      fontFamily="mono"
      fontSize="xs"
      shadow="card"
      minW="640px"
    >
      <Box display="grid" gridTemplateColumns="200px 1fr" h="380px">
        {/* Schema tree */}
        <Box
          bg="bg.elevated"
          borderRight="1px solid"
          borderColor="border"
          py={2.5}
        >
          <Text
            px={3}
            pb={1.5}
            fontSize="10px"
            fontWeight="700"
            letterSpacing="wide"
            color="fg.subtle"
          >
            SCHEMA · payments
          </Text>
          {tree.map((r, i) => (
            <HStack
              key={i}
              h="20px"
              pl={`${12 + r.indent * 14}px`}
              pr={3}
              gap={1.5}
              fontWeight={r.bold || r.active ? "600" : "400"}
              color={r.bold || r.active ? "fg" : "fg.muted"}
              bg={r.active ? "bg.subtle" : "transparent"}
              fontSize="11px"
            >
              {r.key && (
                <Box color="warn" display="flex" alignItems="center">
                  <LuKeyRound size={10} />
                </Box>
              )}
              {r.link && (
                <Box color="info" display="flex" alignItems="center">
                  <LuLink size={10} />
                </Box>
              )}
              <Text flex="1" truncate>
                {r.n}
              </Text>
              {r.count && (
                <Text fontSize="10px" color="fg.subtle">
                  {r.count}
                </Text>
              )}
            </HStack>
          ))}
        </Box>

        {/* Result */}
        <Flex direction="column">
          <HStack
            h="32px"
            px={3}
            bg="bg.elevated"
            borderBottom="1px solid"
            borderColor="border"
            fontSize="11px"
            gap={2}
          >
            <Text color="fg" fontWeight="600">
              Result
            </Text>
            <Text color="fg.subtle">·</Text>
            <Text color="ok">3 rows</Text>
            <Text color="fg.subtle">·</Text>
            <Text>14ms</Text>
            <Box flex="1" />
            <Text color="accent.emphasized">EXPLAIN</Text>
          </HStack>
          <Box as="table" w="100%" style={{ borderCollapse: "collapse" }}>
            <Box as="thead">
              <Box as="tr" bg="bg.elevated" color="fg.muted">
                {[
                  "#",
                  "id",
                  "provider_key",
                  "tenant",
                  "active",
                  "created_at",
                ].map((h) => (
                  <Box
                    as="th"
                    key={h}
                    px={2.5}
                    py={1.5}
                    textAlign="left"
                    fontWeight="600"
                    fontSize="10px"
                    borderBottom="1px solid"
                    borderColor="border"
                  >
                    {h}
                  </Box>
                ))}
              </Box>
            </Box>
            <Box as="tbody">
              {rows.map((row, i) => (
                <Box
                  as="tr"
                  key={i}
                  borderBottom="1px solid"
                  borderColor="border.subtle"
                >
                  {row.map((c, j) => (
                    <Box
                      as="td"
                      key={j}
                      px={2.5}
                      py={1.5}
                      color={j === 0 ? "fg.subtle" : "fg"}
                      fontSize="11px"
                      fontFamily="mono"
                    >
                      {c}
                    </Box>
                  ))}
                </Box>
              ))}
            </Box>
          </Box>
          <Box flex="1" />
          <Box
            px={3}
            py={2.5}
            borderTop="1px solid"
            borderColor="border"
            bg="bg.elevated"
            fontSize="10px"
          >
            <Text
              color="fg.subtle"
              fontWeight="700"
              letterSpacing="wide"
              mb={1}
            >
              EXPLAIN ANALYZE
            </Text>
            <Text color="fg.muted" lineHeight="1.7">
              ↳{" "}
              <Text as="span" color="fg">
                Index Scan
              </Text>{" "}
              using{" "}
              <Text as="span" color="accent.emphasized">
                idx_route_provider
              </Text>{" "}
              · 0.42..18.7 ·{" "}
              <Text as="span" color="ok">
                3 rows · 0.21ms
              </Text>
            </Text>
          </Box>
        </Flex>
      </Box>
    </Box>
  );
}

// GitDiffPreview — feature 3: git-native diff.
export function GitDiffPreview() {
  const lines: Array<[string, string, "ctx" | "del" | "add" | "hunk"]> = [
    ["@@", "-42,7 +42,11 @@  ## 3. Verify latency", "hunk"],
    [" ", "", "ctx"],
    ["-", "```http GET {{api}}/v2/health", "del"],
    ["-", "expect: status === 200", "del"],
    ["-", "```", "del"],
    [" ", "", "ctx"],
    ["+", "```http POST {{api}}/v2/payments", "add"],
    ["+", "Authorization: Bearer {{user_token}}", "add"],
    ["+", "Content-Type: application/json", "add"],
    ["+", "", "add"],
    ["+", '{ "amount": 1200, "currency": "BRL" }', "add"],
    ["+", "", "add"],
    ["+", "expect: status === 201", "add"],
    ["+", "expect: time < 500ms", "add"],
    ["+", "capture: payment_id = $.id", "add"],
    ["+", "```", "add"],
  ];
  type Kind = "ctx" | "del" | "add" | "hunk";
  const colorFor = (k: Kind) =>
    k === "add"
      ? "ok"
      : k === "del"
        ? "err"
        : k === "hunk"
          ? "info"
          : "fg.subtle";
  const bgFor = (k: Kind) =>
    k === "add"
      ? "color-mix(in oklch, var(--chakra-colors-ok) 14%, transparent)"
      : k === "del"
        ? "color-mix(in oklch, var(--chakra-colors-err) 14%, transparent)"
        : k === "hunk"
          ? "color-mix(in oklch, var(--chakra-colors-info) 10%, transparent)"
          : "transparent";
  return (
    <Box
      bg="bg.surface"
      border="1px solid"
      borderColor="border"
      rounded="lg"
      p={4}
      fontFamily="mono"
      fontSize="xs"
      shadow="card"
    >
      <HStack
        gap={2}
        fontSize="11px"
        color="fg.muted"
        pb={2.5}
        mb={3}
        borderBottom="1px solid"
        borderColor="border.subtle"
      >
        <HStack gap={1.5} color="fg" fontWeight="600">
          <LuGitBranch size={12} />
          <Text fontFamily="mono">main</Text>
        </HStack>
        <Text color="fg.subtle">·</Text>
        <Text>3 changes · runbooks/payments/rollout-v2.3.md</Text>
        <Box flex="1" />
        <Text color="accent.emphasized" fontWeight="600">
          +18 −4
        </Text>
      </HStack>
      <Box fontFamily="mono" fontSize="11px" lineHeight="1.7">
        {lines.map(([sign, line, kind], i) => (
          <Box key={i} display="grid" gridTemplateColumns="20px 1fr" gap={2}>
            <Text
              color={colorFor(kind)}
              textAlign="center"
              fontWeight="700"
              bg={kind === "hunk" ? bgFor(kind) : "transparent"}
            >
              {sign}
            </Text>
            <Text
              color={
                kind === "add" ? "fg" : kind === "hunk" ? "info" : "fg.muted"
              }
              fontStyle={kind === "hunk" ? "italic" : "normal"}
              bg={bgFor(kind)}
              px={1}
              textDecoration={kind === "del" ? "line-through" : "none"}
              textDecorationColor="var(--chakra-colors-fg-subtle)"
            >
              {line || " "}
            </Text>
          </Box>
        ))}
      </Box>
    </Box>
  );
}
