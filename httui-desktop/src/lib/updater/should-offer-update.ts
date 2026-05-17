/**
 * Client-side update-channel gate (V12 cenário 9).
 *
 * The tauri-updater endpoint points at the GitHub `releases/latest`
 * feed, which already excludes pre-releases server-side. This gate is
 * the second line of defence: even if a feed advertises a pre-release
 * build, a user who has not opted in never gets prompted.
 *
 * A version is treated as a pre-release when its semver carries an
 * `-rc` / `-beta` / `-alpha` identifier, with or without the dot
 * separator (`1.0.0-rc1` and `1.0.0-rc.1` are both pre-releases) so
 * the predicate stays aligned with the release-tag conventions in
 * `docs/RELEASE.md`.
 */
const PRERELEASE_RE = /-(?:rc|beta|alpha)\.?\d+/i;

export function isPrerelease(version: string): boolean {
  return PRERELEASE_RE.test(version);
}

/**
 * Decide whether an available update should be surfaced to the user.
 *
 * @param version  The candidate update version (semver), or nullish
 *                  when the updater reported no update.
 * @param includePrereleases  The persisted opt-in
 *                  (`autoUpdateIncludePrereleases`).
 */
export function shouldOfferUpdate(
  version: string | null | undefined,
  includePrereleases: boolean,
): boolean {
  if (!version) return false;
  if (isPrerelease(version) && !includePrereleases) return false;
  return true;
}
