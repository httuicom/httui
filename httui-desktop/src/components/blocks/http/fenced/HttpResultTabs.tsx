import { Badge, Box, Flex, Spinner, Tabs, Text } from "@chakra-ui/react";
import type { ReactNode } from "react";
import type {
  HttpCookieRaw,
  HttpResponseFull,
  HttpTimingBreakdown,
} from "@/lib/tauri/streamedExecution";
import type { ExecutionState } from "./shared";
import { bodyAsText } from "./shared";

interface HttpResultTabsProps {
  executionState: ExecutionState;
  response: HttpResponseFull | null;
  error: string | null;
  cached: boolean;
  /**
   * Render-prop for the Body tab. Caller decides how to render the body
   * (HttpBodyView in the main panel) — keeps this file free of the deep
   * dependency tree (json visualizer, preview overlay, CM6 viewer, etc).
   */
  bodyView: (
    rawBody: string,
    prettyBody: string,
    response: HttpResponseFull,
  ) => ReactNode;
}

export function HttpResultTabs({
  executionState,
  response,
  error,
  cached,
  bodyView,
}: HttpResultTabsProps) {
  if (executionState === "running") {
    return (
      <Flex align="center" justify="center" py={6} gap={2}>
        <Spinner size="sm" />
        <Text fontSize="sm" color="fg.muted">
          Running request...
        </Text>
      </Flex>
    );
  }
  if (executionState === "error" && error) {
    return (
      <Box px={3} py={3} bg="red.subtle" color="red.fg" fontSize="sm">
        <Text fontWeight="semibold" mb={1}>
          Request failed
        </Text>
        <Text fontFamily="mono" fontSize="xs" whiteSpace="pre-wrap">
          {error}
        </Text>
      </Box>
    );
  }
  if (executionState === "cancelled") {
    return (
      <Box px={3} py={3} fontSize="sm" color="fg.muted">
        <Text>Cancelled</Text>
      </Box>
    );
  }
  if (executionState === "idle" || !response) {
    return (
      <Box px={3} py={3} fontSize="sm" color="fg.subtle">
        <Text>No response yet — press ⌘↵ to run</Text>
      </Box>
    );
  }

  const prettyBody = bodyAsText(response.body);
  const rawBody =
    typeof response.body === "string" ? response.body : prettyBody; // For parsed JSON we don't have the original raw text, so reuse pretty.
  const headerEntries = Object.entries(response.headers);

  return (
    <Box px={2} py={2}>
      <Flex align="center" gap={2} mb={1}>
        <Text
          fontSize="2xs"
          fontWeight="semibold"
          color="fg.muted"
          textTransform="uppercase"
          letterSpacing="wider"
        >
          Response
        </Text>
        {cached && (
          <Badge colorPalette="purple" variant="subtle" size="xs">
            cached
          </Badge>
        )}
      </Flex>
      <Tabs.Root defaultValue="body" size="sm" variant="line">
        <Tabs.List>
          <Tabs.Trigger value="body">Body</Tabs.Trigger>
          <Tabs.Trigger value="headers">
            Headers ({headerEntries.length})
          </Tabs.Trigger>
          <Tabs.Trigger value="cookies">
            Cookies ({response.cookies.length})
          </Tabs.Trigger>
          <Tabs.Trigger value="timing">Timing</Tabs.Trigger>
          <Tabs.Trigger value="raw">Raw</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="body" px={0} pt={2}>
          {bodyView(rawBody, prettyBody, response)}
        </Tabs.Content>
        <Tabs.Content value="headers" px={0} pt={2}>
          <HeadersTab entries={headerEntries} />
        </Tabs.Content>
        <Tabs.Content value="cookies" px={0} pt={2}>
          <HttpCookiesTab cookies={response.cookies} />
        </Tabs.Content>
        <Tabs.Content value="timing" px={0} pt={2}>
          <HttpTimingTab timing={response.timing} />
        </Tabs.Content>
        <Tabs.Content value="raw" px={0} pt={2}>
          <Box
            as="pre"
            fontFamily="mono"
            fontSize="xs"
            whiteSpace="pre-wrap"
            wordBreak="break-word"
            maxH="320px"
            overflowY="auto"
          >
            {`${response.status_code} ${response.status_text}\n` +
              headerEntries.map(([k, v]) => `${k}: ${v}`).join("\n") +
              "\n\n" +
              prettyBody}
          </Box>
        </Tabs.Content>
      </Tabs.Root>
    </Box>
  );
}

// ─────────────────────── Headers tab ───────────────────────

function HeadersTab({ entries }: { entries: [string, string][] }) {
  if (entries.length === 0) {
    return (
      <Text fontSize="xs" color="fg.muted">
        (no headers)
      </Text>
    );
  }
  return (
    <Box as="table" fontFamily="mono" fontSize="xs" w="100%">
      <Box as="tbody">
        {entries.map(([k, v]) => (
          <Box as="tr" key={k}>
            <Box
              as="td"
              pr={3}
              py={0.5}
              color="fg.muted"
              verticalAlign="top"
              whiteSpace="nowrap"
            >
              {k}
            </Box>
            <Box as="td" py={0.5} wordBreak="break-all">
              {v}
            </Box>
          </Box>
        ))}
      </Box>
    </Box>
  );
}

// ─────────────────────── Cookies tab ───────────────────────

function HttpCookiesTab({ cookies }: { cookies: HttpCookieRaw[] }) {
  if (cookies.length === 0) {
    return (
      <Text fontSize="xs" color="fg.muted">
        (no Set-Cookie headers in this response)
      </Text>
    );
  }
  return (
    <Box as="table" fontFamily="mono" fontSize="xs" w="100%">
      <Box as="thead">
        <Box as="tr" color="fg.muted">
          {["Name", "Value", "Domain", "Path", "Expires", "Flags"].map((h) => (
            <Box
              as="th"
              key={h}
              pr={3}
              py={1}
              textAlign="left"
              fontWeight="semibold"
            >
              {h}
            </Box>
          ))}
        </Box>
      </Box>
      <Box as="tbody">
        {cookies.map((c, i) => (
          <Box as="tr" key={`${c.name}-${i}`}>
            <Box as="td" pr={3} py={0.5}>
              {c.name}
            </Box>
            <Box as="td" pr={3} py={0.5} wordBreak="break-all">
              {c.value}
            </Box>
            <Box as="td" pr={3} py={0.5}>
              {c.domain ?? "—"}
            </Box>
            <Box as="td" pr={3} py={0.5}>
              {c.path ?? "—"}
            </Box>
            <Box as="td" pr={3} py={0.5}>
              {c.expires ?? "—"}
            </Box>
            <Box as="td" pr={3} py={0.5} color="fg.muted">
              {[c.secure && "Secure", c.http_only && "HttpOnly"]
                .filter(Boolean)
                .join(" · ") || "—"}
            </Box>
          </Box>
        ))}
      </Box>
    </Box>
  );
}

// ─────────────────────── Timing tab ───────────────────────

function HttpTimingTab({ timing }: { timing: HttpTimingBreakdown }) {
  // V1 ships only `total_ms`; sub-fields are reserved for a follow-up that
  // wires reqwest connect/TLS/TTFB hooks. Show them when present, otherwise
  // an explanatory line.
  const segments: Array<{ label: string; ms: number | null | undefined }> = [
    { label: "DNS", ms: timing.dns_ms },
    { label: "Connect", ms: timing.connect_ms },
    { label: "TLS", ms: timing.tls_ms },
    { label: "TTFB", ms: timing.ttfb_ms },
  ];
  const hasBreakdown = segments.some((s) => s.ms != null);
  const total = timing.total_ms || 0;
  return (
    <Box>
      <Flex align="center" gap={2} mb={2}>
        <Text fontFamily="mono" fontSize="xs" color="fg.muted">
          Total
        </Text>
        <Box flex={1} h="6px" borderRadius="sm" bg="blue.500" minW="6px" />
        <Text fontFamily="mono" fontSize="xs">
          {total}ms
        </Text>
      </Flex>
      {hasBreakdown ? (
        segments
          .filter((s) => s.ms != null)
          .map((s) => {
            const w = total > 0 ? Math.max(2, (s.ms! / total) * 100) : 0;
            return (
              <Flex key={s.label} align="center" gap={2} mb={1}>
                <Text
                  fontFamily="mono"
                  fontSize="xs"
                  color="fg.muted"
                  minW="56px"
                >
                  {s.label}
                </Text>
                <Box
                  h="4px"
                  borderRadius="sm"
                  bg="cyan.400"
                  w={`${w}%`}
                  minW="4px"
                />
                <Text fontFamily="mono" fontSize="xs" color="fg.muted">
                  {s.ms}ms
                </Text>
              </Flex>
            );
          })
      ) : (
        <Text fontSize="xs" color="fg.subtle" mt={2}>
          DNS / Connect / TLS / TTFB breakdown will appear here once the
          executor exposes them. Total time only for now.
        </Text>
      )}
    </Box>
  );
}
