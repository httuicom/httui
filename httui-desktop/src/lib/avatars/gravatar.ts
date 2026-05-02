// V2 / cenário 4.5 — Gravatar URL helper for the DocHeader author chip.
//
// Gravatar requires the MD5 of the lower-cased trimmed email. Returns
// `null` for empty / undefined input so consumers can `if (!url)
// return <Fallback />` without juggling the empty-string edge.

import { md5 } from "./md5";

const GRAVATAR_BASE = "https://www.gravatar.com/avatar";

export interface GravatarOptions {
  /** Display size in CSS pixels. We request 2x for retina sharpness. */
  size?: number;
  /**
   * Fallback strategy when no Gravatar exists. Default `404` lets the
   * caller render its own fallback (the colored-initials circle); use
   * `identicon` for an automatic Gravatar-supplied fallback image.
   */
  fallback?: "404" | "identicon" | "blank" | "mp" | "retro" | "robohash";
}

/**
 * Build a Gravatar avatar URL for an email. Returns `null` when the
 * email is missing or whitespace-only.
 */
export function gravatarUrl(
  email: string | null | undefined,
  opts: GravatarOptions = {},
): string | null {
  const trimmed = email?.trim().toLowerCase();
  if (!trimmed) return null;
  const hash = md5(trimmed);
  const size = opts.size ?? 40;
  const fallback = opts.fallback ?? "404";
  const params = new URLSearchParams({ s: String(size * 2), d: fallback });
  return `${GRAVATAR_BASE}/${hash}?${params.toString()}`;
}
