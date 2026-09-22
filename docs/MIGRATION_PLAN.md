# Migration plan: local pilot → habitual use

**Status:** draft · **Owner:** @Hrirks · **Verified against:** `main` @ `ebbc45b`
**Tracking issue:** the P0 epic

## Why this plan exists

The server is being wired into day-to-day agent workflows (OpenCode config, `AGENTS.md`
guidance to prefer `query_context` over grep). Two of the gaps below mean that "prefer
the server" currently degrades development quality rather than improving it: retrieval
can return **stale lines under fresh-looking metadata**, and **test directories are
invisible**. Everything else on the list holds too.

While those are open, the honest posture is: local, read-only pilot only — not
habitual or automatic use.

## Verified baseline (main @ `ebbc45b`)

| # | Gap | Verdict | Evidence |
|---|-----|---------|----------|
| 1 | Stale index, no staleness signal | Confirmed, understated | `graph_memory_service.rs:524,831,892` + `slice_lines:1283` |
| 2 | Test dirs excluded by design | Confirmed | `chunker.rs:18-31` |
| 3 | No filesystem trust boundary | Confirmed | `enhanced_context_server.rs:3060-3090`, zero `canonicalize` |
| 4 | At-rest + embedding egress unprotected | Confirmed | `main.rs:21-33`, `container.rs:65-131` |
| 5 | Governance context invisible | Confirmed (Java is supported too) | `languages.rs` |
| 6 | Over-broad tool surface | Confirmed, 40 tools | `enhanced_context_server.rs` |
| 7 | No prompt-injection handling | Confirmed absent | 0 hits for provenance/untrusted |
| 8 | No retrieval benchmark | Confirmed | no `benches/`, no criterion |

## Rollout gates

- **Gate 0 — local read-only pilot. CURRENT.** Nothing on this list blocks it.
- **Gate 1 — habitual/automatic use.** Requires P0 items 1-4 closed, plus the eval
  harness (item 8) reporting a stale-result rate of 0 and a recall baseline.
- **Gate 2 — proprietary code at rest, multi-repo, or a non-loopback embedding
  endpoint.** Requires items 4 and 7 signed off plus an explicit decision on
  `OLLAMA_BASE_URL` egress.

## Phases

### Phase 0 — enablers (no behaviour change)
- Schema evolution through `ADDED_COLUMNS` + `SCHEMA_VERSION` in `src/db/init.rs`
  (append entries, never edit earlier ones).
- One config resolution point alongside the existing `container.rs` helpers.
- Caveat (or gate) the `AGENTS.md` "prefer the server" guidance behind Gate 1.

### Phase 1 — P0 (blocks habitual use)
Items 1-4. Details and acceptance criteria in the linked issues.

### Phase 2 — P1 (quality and hardening)
Items 5-8.

## Back-compatibility strategy

- Existing databases gain new columns with defaults via the `ADDED_COLUMNS` path;
  `PRAGMA user_version` recognises databases written by older builds.
- `indexed_files` backfill: a file with no recorded hash is treated as stale on first
  read, forcing exactly one re-index. Acceptable, but must be logged.
- If per-project storage is adopted (item 4, D3), ship a one-way `--migrate-storage`
  command and keep single-database mode available for one release.

## Verification

- `cargo fmt` / `clippy` / `test` green on every change (existing 142 tests).
- New regression tests beside each fix: the stale-read path, test inclusion and
  ranking, trust-boundary rejection, and file permissions.
- The eval harness (item 8) reports: recall@k, stale-result rate, response tokens,
  rg-substitution rate.

## Non-goals

- No new tree-sitter grammars in this migration (item 5 imports governance documents,
  not new code languages).
- No cloud or remote embedding backends.
- No multi-user server mode.

## Open decisions

| ID | Decision | Recommended default |
|----|----------|---------------------|
| D1 | Stale handling: verify-and-flag vs auto-reindex-first | Flag first, then auto-reindex before retrieval |
| D2 | Root policy: default-deny vs warn-and-proceed | Default-deny with explicit registration |
| D3 | Storage: per-project DB files vs one DB + namespaces | Per-project file under `projects/<id>.db` |
| D4 | `AGENTS.md` guidance: keep, caveat, or gate | Caveat now, restore after Gate 1 |
