// HTTP block response-body rendering: pretty/raw/visualize switch,
// the CM6 read-only viewer, the image/PDF/HTML preview card, and the
// fullscreen preview overlay.
//
// Extracted verbatim from HttpFencedPanel.tsx (A1 / audit 03 §1 seam
// #2). The orchestrator consumes only `HttpBodyView`; `detectPreview`,
// `selectBodyLanguage` and `detectLang` are also exported so the pure
// content-type/heuristic logic can be unit-tested (the panel had ~no
// coverage). Everything else is module-internal.

import { useEffect, useMemo, useRef, useState } from "react";
import {
  Box,
  Button,
  Flex,
  HStack,
  IconButton,
  Portal,
  Text,
} from "@chakra-ui/react";
import {
  LuClipboard,
  LuExpand,
  LuFileText,
  LuGlobe,
  LuX,
} from "react-icons/lu";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { syntaxHighlighting } from "@codemirror/language";
import { oneDarkHighlightStyle } from "@codemirror/theme-one-dark";
import { json } from "@codemirror/lang-json";
import { xml } from "@codemirror/lang-xml";
import { html } from "@codemirror/lang-html";

import type { HttpResponseFull } from "@/lib/tauri/streamedExecution";

import { formatBytes } from "./shared";
import {
  HttpJsonVisualizer,
  parseJsonForVisualize,
} from "./HttpJsonVisualizer";

// "pretty" routes by content-type: image/pdf/html → visual preview;
// everything else → CM6 read-only viewer with syntax highlight. The
// dedicated "preview" mode was folded into pretty (it duplicated work).
type BodyViewMode = "pretty" | "raw" | "visualize";

// ─────────── CM6 read-only body viewer (Onda 4) ───────────
// Replaces the old `<Box as="pre" dangerouslySetInnerHTML>` + lowlight
// renderer. CM6 paints incrementally even on multi-MB bodies, so the webview
// stops blocking on large responses. Pattern mirrors `StandaloneBlock.tsx`
// (`src/components/blocks/standalone/StandaloneBlock.tsx:102-163`).

const cmReadOnlyBodyTheme = EditorView.theme({
  "&": { fontSize: "12px", maxHeight: "320px" },
  ".cm-content": {
    fontFamily: "var(--chakra-fonts-mono)",
    padding: "8px",
  },
  ".cm-gutters": { display: "none" },
  ".cm-scroller": { overflow: "auto", overscrollBehavior: "contain" },
  ".cm-activeLine": { backgroundColor: "transparent" },
});
const cmBodyReadOnly = EditorState.readOnly.of(true);

/** Pick a CM6 language extension based on the response Content-Type, with
 * a JSON/XML heuristic fallback when the header is missing or generic. */
export function selectBodyLanguage(
  contentType: string | null,
  text: string,
): Extension | null {
  if (contentType) {
    const ct = contentType.split(";")[0].trim().toLowerCase();
    if (ct.includes("json")) return json();
    if (ct.includes("xml") || ct.includes("svg")) return xml();
    if (ct.includes("html")) return html();
  }
  const heuristic = detectLang(text, "pretty");
  if (heuristic === "json") return json();
  if (heuristic === "xml") return xml();
  return null;
}

function HttpBodyCM6Viewer({
  text,
  contentType,
}: {
  text: string;
  contentType: string | null;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const lang = selectBodyLanguage(contentType, text);
    const extensions: Extension[] = [
      cmBodyReadOnly,
      cmReadOnlyBodyTheme,
      syntaxHighlighting(oneDarkHighlightStyle),
      ...(lang ? [lang] : []),
    ];
    const view = new EditorView({
      state: EditorState.create({ doc: text, extensions }),
      parent: containerRef.current,
    });
    return () => {
      view.destroy();
    };
  }, [text, contentType]);

  return (
    <Box
      ref={containerRef}
      border="1px solid"
      borderColor="border"
      rounded="md"
      overflow="hidden"
    />
  );
}

export function HttpBodyView({
  rawBody,
  prettyBody,
  response,
}: {
  rawBody: string;
  prettyBody: string;
  response: HttpResponseFull;
}) {
  const [view, setView] = useState<BodyViewMode>("pretty");

  const previewMeta = useMemo(() => detectPreview(response), [response]);
  const visualizeData = useMemo(
    () => parseJsonForVisualize(prettyBody),
    [prettyBody],
  );

  const text = view === "pretty" ? prettyBody : rawBody;

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      /* noop */
    }
  };

  return (
    <>
      <HStack gap={1} mb={1}>
        <Button
          size="2xs"
          variant={view === "pretty" ? "solid" : "ghost"}
          onClick={() => setView("pretty")}
        >
          pretty
        </Button>
        <Button
          size="2xs"
          variant={view === "raw" ? "solid" : "ghost"}
          onClick={() => setView("raw")}
        >
          raw
        </Button>
        {visualizeData !== null && (
          <Button
            size="2xs"
            variant={view === "visualize" ? "solid" : "ghost"}
            onClick={() => setView("visualize")}
          >
            ⊞ visualize
          </Button>
        )}
        <Box flex={1} />
        {(view === "pretty" || view === "raw") && (
          <IconButton
            aria-label="Copy body"
            size="2xs"
            variant="ghost"
            onClick={onCopy}
          >
            <LuClipboard />
          </IconButton>
        )}
      </HStack>
      {view === "pretty" && previewMeta.kind !== "none" && (
        <HttpBodyPreview meta={previewMeta} sizeBytes={response.size_bytes} />
      )}
      {view === "visualize" && visualizeData !== null && (
        <HttpJsonVisualizer data={visualizeData} />
      )}
      {((view === "pretty" && previewMeta.kind === "none") || view === "raw") &&
        (text ? (
          <HttpBodyCM6Viewer
            text={text}
            // `pretty` view picks lang from the response Content-Type;
            // `raw` view shows the bytes verbatim with no highlight (avoids
            // distorting non-pretty payloads like form-urlencoded).
            contentType={
              view === "pretty"
                ? (response.headers["content-type"] ??
                  response.headers["Content-Type"] ??
                  null)
                : null
            }
          />
        ) : (
          <Box as="pre" fontFamily="mono" fontSize="xs" color="fg.muted">
            (empty body)
          </Box>
        ))}
    </>
  );
}

// ─────────────────────── Preview (image/PDF/HTML) ───────────────────────

type PreviewMeta =
  | { kind: "none" }
  | { kind: "image"; dataUrl: string; alt: string }
  | { kind: "pdf"; dataUrl: string }
  | { kind: "html"; html: string };

export function detectPreview(response: HttpResponseFull): PreviewMeta {
  const ctRaw =
    response.headers["content-type"] ?? response.headers["Content-Type"] ?? "";
  const ct = ctRaw.split(";")[0].trim().toLowerCase();
  const body = response.body;

  // Binary base64 — image or PDF
  if (
    typeof body === "object" &&
    body !== null &&
    "encoding" in body &&
    (body as Record<string, unknown>).encoding === "base64"
  ) {
    const data = String((body as Record<string, unknown>).data ?? "");
    if (ct.startsWith("image/")) {
      return { kind: "image", dataUrl: `data:${ct};base64,${data}`, alt: ct };
    }
    if (ct === "application/pdf") {
      return { kind: "pdf", dataUrl: `data:application/pdf;base64,${data}` };
    }
    return { kind: "none" };
  }

  // HTML — rendered in a sandboxed iframe (no scripts).
  if (ct === "text/html" && typeof body === "string") {
    return { kind: "html", html: body };
  }

  return { kind: "none" };
}

/**
 * Inline preview affordance per content kind:
 *   - **image**: rendered inline (no scroll-leak issue) with an "expand"
 *     IconButton overlay in the top-right that opens the fullscreen modal.
 *   - **pdf** / **html**: a richer placeholder card (icon + type + size +
 *     CTA) that opens the modal on click — the modal is required because
 *     iframe wheel events bypass DOM scroll containment in the Tauri
 *     webview and would otherwise leak into the markdown editor.
 */
function HttpBodyPreview({
  meta,
  sizeBytes,
}: {
  meta: PreviewMeta;
  sizeBytes: number;
}) {
  // Lifecycle: HTML preview uses a blob URL we must revoke on unmount.
  const [blobUrl, setBlobUrl] = useState<string | null>(null);
  useEffect(() => {
    if (meta.kind !== "html") {
      setBlobUrl(null);
      return;
    }
    const url = URL.createObjectURL(
      new Blob([meta.html], { type: "text/html" }),
    );
    setBlobUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [meta]);

  const [open, setOpen] = useState(false);

  if (meta.kind === "none") {
    return (
      <Text fontSize="xs" color="fg.muted">
        Preview not available for this response.
      </Text>
    );
  }

  const label =
    meta.kind === "image"
      ? "Image preview"
      : meta.kind === "pdf"
        ? "PDF preview"
        : "HTML preview";

  // Image renders inline — no internal scroll, so no leak. The expand
  // button still gives access to the fullscreen viewer for big images.
  if (meta.kind === "image") {
    return (
      <>
        <Box
          position="relative"
          bg="bg.subtle"
          borderWidth="1px"
          borderColor="border"
          borderRadius="sm"
          p={2}
          display="flex"
          justifyContent="center"
          alignItems="center"
          maxH="400px"
          overflow="hidden"
        >
          <img
            src={meta.dataUrl}
            alt={meta.alt}
            style={{
              maxWidth: "100%",
              maxHeight: "380px",
              objectFit: "contain",
              display: "block",
            }}
          />
          <IconButton
            aria-label="Open image fullscreen"
            size="xs"
            variant="solid"
            onClick={() => setOpen(true)}
            position="absolute"
            top={2}
            right={2}
            opacity={0.85}
            _hover={{ opacity: 1 }}
          >
            <LuExpand />
          </IconButton>
        </Box>
        {open && (
          <PreviewOverlay
            meta={meta}
            blobUrl={blobUrl}
            label={label}
            onClose={() => setOpen(false)}
          />
        )}
      </>
    );
  }

  // PDF / HTML — richer placeholder card with icon + type + size + CTA.
  const Icon = meta.kind === "pdf" ? LuFileText : LuGlobe;
  const typeLine = meta.kind === "pdf" ? "PDF document" : "HTML page";

  return (
    <>
      <Box
        bg="bg.subtle"
        borderWidth="1px"
        borderColor="border"
        borderRadius="md"
        px={4}
        py={3}
        display="flex"
        alignItems="center"
        gap={3}
      >
        <Box
          display="flex"
          alignItems="center"
          justifyContent="center"
          w="40px"
          h="40px"
          borderRadius="sm"
          bg="bg.panel"
          color="fg.muted"
          flexShrink={0}
        >
          <Box fontSize="20px">
            <Icon />
          </Box>
        </Box>
        <Box flex={1} display="flex" flexDirection="column" gap={0.5} minW={0}>
          <Text fontSize="sm" fontWeight="medium">
            {typeLine}
          </Text>
          <Text fontSize="xs" color="fg.muted">
            {formatBytes(sizeBytes)} · click to open in a focused viewer
          </Text>
        </Box>
        <Button
          size="sm"
          variant="outline"
          onClick={() => setOpen(true)}
          disabled={meta.kind === "html" && !blobUrl}
          flexShrink={0}
        >
          <LuExpand /> Open
        </Button>
      </Box>
      {open && (
        <PreviewOverlay
          meta={meta}
          blobUrl={blobUrl}
          label={label}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}

/**
 * Fullscreen preview modal — Portal + Box (deliberately not Chakra Dialog,
 * which would steal focus from the markdown editor on close). Locks body
 * scroll while open so wheel events that escape from data: URL iframes
 * have nowhere to land. Esc + backdrop click + close button all dismiss.
 */
function PreviewOverlay({
  meta,
  blobUrl,
  label,
  onClose,
}: {
  meta: PreviewMeta;
  blobUrl: string | null;
  label: string;
  onClose: () => void;
}) {
  // Lock body scroll while open. The Tauri webview's compositor scroll
  // routing means wheel leaks out of the iframe — locking the body
  // prevents the host doc from scrolling beneath the modal.
  useEffect(() => {
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = prevOverflow;
    };
  }, []);

  // Esc closes the modal. Window-level so it works regardless of focus
  // (the iframe steals focus on its own).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <Portal>
      <Box
        position="fixed"
        inset={0}
        bg="blackAlpha.700"
        zIndex={3000}
        display="flex"
        alignItems="center"
        justifyContent="center"
        p={6}
        onClick={onClose}
        role="dialog"
        aria-modal="true"
        backdropFilter="blur(2px)"
      >
        <Box
          bg="bg.panel"
          borderWidth="1px"
          borderColor="border"
          borderRadius="lg"
          boxShadow="2xl"
          w="90vw"
          h="90vh"
          maxW="1400px"
          display="flex"
          flexDirection="column"
          overflow="hidden"
          onClick={(e) => e.stopPropagation()}
        >
          <Flex
            justify="space-between"
            align="center"
            px={4}
            py={2.5}
            bg="bg.subtle"
            borderBottomWidth="1px"
            borderColor="border"
          >
            <Text fontSize="sm" fontWeight="semibold">
              {label}
            </Text>
            <IconButton
              aria-label="Close preview"
              size="xs"
              variant="ghost"
              onClick={onClose}
            >
              <LuX />
            </IconButton>
          </Flex>
          <Box flex={1} overflow="hidden" bg="bg" p={4}>
            {meta.kind === "image" && (
              <Box
                w="100%"
                h="100%"
                display="flex"
                alignItems="center"
                justifyContent="center"
                bg="bg.subtle"
                borderRadius="md"
              >
                <img
                  src={meta.dataUrl}
                  alt={meta.alt}
                  style={{
                    maxWidth: "100%",
                    maxHeight: "100%",
                    objectFit: "contain",
                  }}
                />
              </Box>
            )}
            {meta.kind === "pdf" && (
              <iframe
                src={meta.dataUrl}
                title="PDF preview"
                style={{
                  width: "100%",
                  height: "100%",
                  border: "1px solid var(--chakra-colors-border)",
                  borderRadius: "var(--chakra-radii-md)",
                  display: "block",
                  background: "white",
                }}
              />
            )}
            {meta.kind === "html" && blobUrl && (
              <iframe
                src={blobUrl}
                // `sandbox=""` (empty value) is the strictest policy: no
                // scripts, no forms, no same-origin, no popups. Layout-
                // only rendering.
                sandbox=""
                title="HTML preview"
                style={{
                  width: "100%",
                  height: "100%",
                  border: "1px solid var(--chakra-colors-border)",
                  borderRadius: "var(--chakra-radii-md)",
                  display: "block",
                  background: "white",
                }}
              />
            )}
          </Box>
        </Box>
      </Box>
    </Portal>
  );
}

export function detectLang(
  text: string,
  view: "pretty" | "raw",
): string | null {
  // Pretty mode: try JSON first (most common), fall back to xml/html on
  // angle-bracket starts. Raw mode: trust the bytes — same heuristic.
  void view;
  const trimmed = text.trimStart();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      JSON.parse(trimmed);
      return "json";
    } catch {
      // fall through
    }
  }
  if (trimmed.startsWith("<")) return "xml";
  return null;
}
