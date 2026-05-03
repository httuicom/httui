// Epic 50 Story 01 + 02 — pure derivations for the DocHeader card.
//
// The actual frontmatter parser is Epic 52's job; this module accepts
// already-parsed `Frontmatter` plus the source pieces (firstHeading,
// filePath) and answers questions the card UI asks: what's the title?
// what does the breadcrumb look like? does the abstract need
// fade-out?

import type { PreflightItem } from "@/lib/blocks/preflight-item";

export interface DocHeaderFrontmatter {
  title?: string;
  abstract?: string;
  tags?: ReadonlyArray<string>;
  /** V2 / cenário 4.5 / M6 — pre-flight checklist items. The DocHeader
   *  Checklist subcomponent reads this to render its rows. */
  preflight?: ReadonlyArray<PreflightItem>;
  /** V6 / cenário 6 — user-visible parse error from the slice-1 YAML
   *  parser. The DocHeader card surfaces a "frontmatter invalid" badge
   *  when this is set so the user has a visible signal that their YAML
   *  didn't apply (unterminated block, block-list shape, etc.). */
  error?: string;
}

/**
 * Title resolution priority per Story 02 task #3:
 *   1. Frontmatter `title:` (when non-empty after trimming)
 *   2. The file's first `#` heading
 *   3. The filename without its extension
 */
export function pickH1Title(
  frontmatter: DocHeaderFrontmatter | null,
  firstHeading: string | null,
  filePath: string,
): string {
  const fmTitle = frontmatter?.title?.trim();
  if (fmTitle && fmTitle.length > 0) return fmTitle;
  const fh = firstHeading?.trim();
  if (fh && fh.length > 0) return fh;
  return filenameWithoutExtension(filePath);
}

export function filenameWithoutExtension(filePath: string): string {
  // Use the last `/` in either separator family — vaults are POSIX
  // by convention but Windows backslashes can leak in via paste.
  const lastSlash = Math.max(filePath.lastIndexOf("/"), filePath.lastIndexOf("\\"));
  const base = lastSlash === -1 ? filePath : filePath.slice(lastSlash + 1);
  const dotIdx = base.lastIndexOf(".");
  return dotIdx <= 0 ? base : base.slice(0, dotIdx);
}

export interface BreadcrumbSegment {
  label: string;
  /** Absolute-from-vault-root path that selects this segment when
   *  clicked. Empty path = root segment. */
  path: string;
}

/**
 * Build a vault-root-anchored breadcrumb from a relative file path.
 * Example: `notes/runbooks/db.md` →
 *   [{ label: "notes", path: "notes" },
 *    { label: "runbooks", path: "notes/runbooks" },
 *    { label: "db", path: "notes/runbooks/db.md" }]
 *
 * The leaf segment uses the filename WITH its extension when there's
 * one; the consumer typically renders it as the active (non-clickable)
 * segment.
 */
export function deriveBreadcrumb(
  relativeFilePath: string,
): BreadcrumbSegment[] {
  const cleaned = relativeFilePath.replace(/^\/+/u, "").replace(/\\/gu, "/");
  if (cleaned.length === 0) return [];
  const parts = cleaned.split("/").filter((p) => p.length > 0);
  const out: BreadcrumbSegment[] = [];
  let acc = "";
  for (let i = 0; i < parts.length; i++) {
    acc = acc.length === 0 ? parts[i]! : `${acc}/${parts[i]!}`;
    const isLeaf = i === parts.length - 1;
    out.push({
      label: isLeaf ? stripExtension(parts[i]!) : parts[i]!,
      path: acc,
    });
  }
  return out;
}

function stripExtension(name: string): string {
  const dotIdx = name.lastIndexOf(".");
  return dotIdx <= 0 ? name : name.slice(0, dotIdx);
}

export interface AbstractDisplay {
  text: string;
  /** True when the abstract is long enough that a "more" toggle
   *  helps; the consumer applies a CSS clamp + button below. */
  needsTruncation: boolean;
}

export const ABSTRACT_FADE_THRESHOLD = 250;

export function deriveAbstractDisplay(
  frontmatter: DocHeaderFrontmatter | null,
): AbstractDisplay | null {
  const raw = frontmatter?.abstract?.trim();
  if (!raw || raw.length === 0) return null;
  return {
    text: raw,
    needsTruncation: raw.length > ABSTRACT_FADE_THRESHOLD,
  };
}
