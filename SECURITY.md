# Security policy

httui is pre-v1. The codebase is being actively reworked, so security
fixes ship to `main`; there is no LTS branch yet.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security reports.
Use one of the following channels instead:

- E-mail [joao@httui.com](mailto:joao@httui.com) with the subject
  prefix `[security]`.
- Open a private advisory via GitHub:
  <https://github.com/httuicom/httui/security/advisories/new>.

Include in your report:

- A description of the issue and its impact.
- Reproduction steps (or a proof-of-concept), and the affected
  commit / version.
- Any suggested mitigation, if you have one.

If you'd like an encrypted channel, mention it in your first message
and we'll set one up.

## Expected response

- Initial acknowledgement within **5 business days**.
- A coordinated fix and disclosure timeline once the report is
  triaged. Most issues are handled within two weeks; complex ones
  may take longer and we'll keep you updated.
- Credit in the release notes (and `CHANGELOG.md`) once the fix
  ships, unless you ask to remain anonymous.

There is no bug bounty program.

## Scope

In scope:

- `httui-desktop` (Tauri app: Rust backend + React frontend)
- `httui-tui` (terminal binary)
- `httui-core` (shared Rust crate)
- `httui-mcp` (MCP server)
- `httui-sidecar` (Node.js Claude Agent SDK bridge)
- `httui-web` (marketing landing — only for issues in the deployed
  site or its build output)

Out of scope:

- Vulnerabilities in upstream dependencies — please report them to
  the upstream project. We'll bump the dependency once a fix is
  available.
- Findings that require physical access to an unlocked machine,
  social-engineering of the user, or already-rooted environments.
- Self-host instances modified by third parties.

## Supported versions

Until v1 ships, only the latest commit on `main` is supported. Once
v1 is tagged, the table below will be updated with the supported
release line(s).

| Version  | Supported          |
| -------- | ------------------ |
| `main`   | :white_check_mark: |
| pre-v1   | :white_check_mark: |
