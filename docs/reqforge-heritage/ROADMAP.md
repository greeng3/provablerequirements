# ReqForge Implementation Roadmap

## Purpose

This document is the implementation companion to
[INTENTIONS.md](INTENTIONS.md) and the formal requirements under
[requirements-doorstop/](requirements-doorstop/). Where INTENTIONS.md
and the requirements say **what** ReqForge does, this roadmap says
**in what order it gets built**, how the work is split across
branches, and which decisions need to be locked down before real
coding starts.

The intended reader is the implementer (currently the author). It is
a working document: update it as phases complete, branches land, and
decisions shift.

## Guiding Principles

- **Vertical slices, not horizontal layers.** Each phase is a thin
  end-to-end cut — back-end + API + front-end + just enough tests —
  that produces something you can run and exercise. No phase sits
  purely on the storage layer or purely on the UI.
- **Thin branch scope.** A branch corresponds to one Work Item and
  one merge. If a phase is too big for a single branch, it splits
  into sub-phases (`1a`, `1b`, `1c`). No branches-off-branches: a
  branch lands on main before the next branches off it.
- **Front-load risk checks.** Before committing deeply to a design
  decision, run a cheap sanity check. Five specific early checks
  apply to this project; they live in a later section of this doc.
- **Dogfood as soon as possible.** Phase 4 is the dogfooding
  milestone — ReqForge becomes capable of managing its own
  requirements. From that point, the
  [`requirements-doorstop/`](requirements-doorstop/) bootstrap can be
  retired.
- **Keep main green.** Every phase-merge leaves `main` buildable,
  testable, and runnable. Treat that as a hard rule even when solo:
  the habit pays off if the project ever gains collaborators.

## Architectural Picks

These are settled for the initial version. The rationale for each is
in the discussion that led to them; listing them here saves re-
deriving on every branch.

### Back-end

- **Language:** Rust (per `TECH-rustBackend`).
- **HTTP framework:** `axum` (Tokio-based; standard in 2026).
- **Logging:** `tracing` + `tracing-subscriber` with a JSON layer,
  writing to stdout (per `TECH-observability`).
- **Git read access:** `gitoxide` (pure-Rust, read-only use — per
  `DEPLOY-noGitOps`, `UX-diffView`). Never shell-out to the git CLI.
- **Search index:** `tantivy` (pure-Rust, Lucene-style). In-memory
  for the initial version (per `UX-search`, `TRACE-uuidIndex`).
- **JSON:** `serde` + `serde_json`. JSON files are pretty-printed
  with two-space indentation on write (per `FORMAT-fileEncoding`).
- **UUIDs:** `uuid` crate, v7 values (per `FORMAT-artifactMetadataSchema`).

### Front-end

- **Language / build:** React + TypeScript, built with Vite.
- **Routing:** `react-router-dom`.
- **Server state:** `@tanstack/react-query`.
- **Styling:** `Tailwind CSS`.
- **Markdown renderer:** `react-markdown` + `remark-gfm` +
  `rehype-highlight` for code blocks.
- **Markdown editor:** `CodeMirror 6` with the Markdown language
  package (per `UX-markdownEditor`).
- **Graph canvas:** `React Flow` (per `UX-linkCreationGraph`).
- **Virtualised lists / matrix:** `@tanstack/react-virtual` (per
  `UX-largeListRendering`, `UX-linkCreationMatrix`).

### Container and ops

- **Base image:** Wolfi (`cgr.dev/chainguard/wolfi-base` for the
  runtime, or `cgr.dev/chainguard/static` if the Rust binary is
  static-linked with musl). Chainguard's Rust and Node dev images
  for build stages. Fallback: Debian-slim if a Phase 5 dependency
  surprises us.
- **Default port:** 36743 (per `DEPLOY-defaultPort`).
- **Binding:** `0.0.0.0` (per `DEPLOY-networkBinding`).
- **Thumbnailer tools** (Phase 5 onward): LibreOffice headless for
  Office documents, ImageMagick / libvips for additional image
  formats (per `UX-uploadPreview`).

### Test infrastructure

- **Back-end unit / integration:** `cargo test` + `cargo nextest`.
- **Front-end unit / component:** `vitest` + React Testing Library.
- **End-to-end:** `selenium-webdriver` (JS) targeting an external
  `selenium-chrome` container. Hook into `make test-e2e` so it can
  be skipped on hosts without the container.

## Branch Strategy

- **One Work Item per branch.** Work Item title conforms to
  `Phase <N><letter?> <descriptive slug>`. GitLab's numeric ID
  forms the branch-name prefix; the full branch name ends up like
  `17-phase-5-non-content-artifact-shapes`.
- **No branches-off-branches.** Each branch reaches a reasonable
  stopping point, merges to main, and the next branches from main.
- **Phase sizing.** Phases where the task list is large and
  naturally splits (Phase 1, 7, 9, 10, 11) get letter suffixes —
  `1a`, `1b`, `1c`. Phases that are coherent single-shot pieces
  (Phase 2, 3, 4, 8) stay as one branch. Expect some phases to
  split further in practice than currently planned; that's fine.

## Work Item / Branch List

One row per planned branch, in chronological order.

| Phase | Work Item title (→ branch name)                    |
| ----- | -------------------------------------------------- |
| 0     | Phase 0 scaffolding                                |
| 1a    | Phase 1a read-only backend core                    |
| 1b    | Phase 1b read-only React frontend                  |
| 1c    | Phase 1c end-to-end smoke testing                  |
| 2     | Phase 2 artifact CRUD with Markdown editor         |
| 3     | Phase 3 typed link authoring                       |
| 4     | Phase 4 review workflow                            |
| 5     | Phase 5 non-content artifact shapes                |
| 6a    | Phase 6a report classes                            |
| 6b    | Phase 6b report exports                            |
| 7a    | Phase 7a graph canvas view                         |
| 7b    | Phase 7b matrix link view                          |
| 7c    | Phase 7c full-text search with Tantivy             |
| 7d    | Phase 7d browsable title-indexed views             |
| 8     | Phase 8 doorstop import                            |
| 9a    | Phase 9a code traceability scanner                 |
| 9b    | Phase 9b code traceability report integration      |
| 10a   | Phase 10a LLM provider adapters and fallback chain |
| 10b   | Phase 10b LLM-assisted rename                      |
| 10c   | Phase 10c MCP server for AI coding agents          |
| 11a   | Phase 11a schema migration                         |
| 11b   | Phase 11b sample content onboarding                |
| 11c   | Phase 11c onboarding polish                        |
| 12a   | Phase 12a LLM-assisted post-import link suggestion |
| 12b   | Phase 12b LLM-assisted on-change link suggestion   |
| 13    | Phase 13 in-app LLM configuration                  |

## MVP Definition

**Phases 1–4.** A working text-based requirements management tool
with typed traceability and a review workflow. Already more useful
than doorstop for the target audience. Everything after Phase 4 is
depth — important depth, but MVP is the point where the tool earns
its keep.

## Early Risk-Adjusted Checks

Sanity checks to run before committing deeply to the corresponding
design decisions. Each is cheap to perform early; each is painful to
unwind if it fails later.

1. **JSON-as-YAML frontmatter rendering on GitHub and GitLab**
   (Phase 0 or 1). Write a sample `.md` with a JSON-in-YAML-delimiters
   frontmatter block, push it to a public GitHub repo and a GitLab
   repo, confirm both render it as a YAML-like table and the Markdown
   body renders normally below. If either fails, revisit
   `FORMAT-frontmatterDelimiters` before building on the convention.

   **Status (Phase 1c):** passes on GitLab; GitHub skipped.

   - **GitLab:** verified by the maintainer against
     `.reqforge-workspace/example-test-repos/sample-project/artifacts/requirements/REQ-helloWorld.md`
     on the Phase 1c branch. The `---`-delimited JSON frontmatter
     renders cleanly as YAML-style metadata, and the `# Hello World`
     body renders as a normal Markdown heading. No change needed to
     `FORMAT-frontmatterDelimiters`.
   - **GitHub:** not verified. The project lives on GitLab, so
     GitHub rendering isn't a target. Revisit only if a GitHub
     mirror is ever set up.

2. **Chown-from-`.git` across host OSes** (Phase 2, where writes
   land). Round-trip write-and-read test on Linux, macOS, and Windows
   Docker hosts. Confirm `REQFORGE_UID` / `REQFORGE_GID` overrides
   behave as specified.

   **Status (Phase 2):** Passed on all three Docker host
   platforms (Linux, macOS, Windows + Docker Desktop) via live
   container round-trip. Repro is scripted — each host runs one
   command end-to-end (build image if absent, stage a fixture
   repo, run Test A default-chown and Test B
   `REQFORGE_UID=4242` override, record the observed UID/GIDs,
   tear down). Each run ends with a summary line ready to paste
   into this block.

   - **Ubuntu / macOS / Windows WSL2 Ubuntu:**
     `./scripts/risk-check-2/risk-check-2.sh`. Auto-detects the
     platform; on WSL2 the repo must live inside WSL's ext4
     (not `/mnt/c/...`) or the script bails.
   - **Windows + Docker Desktop:**
     `.\scripts\risk-check-2\risk-check-2.ps1` from a PowerShell
     terminal inside the repo.

   Both scripts read `.reqforge-workspace/`-independent fixtures
   out of `$TMPDIR` / `$env:TEMP`, never touch the live repo,
   and clean up on exit (including Ctrl-C). See the header
   comment in each script for the precise payloads and
   expected-result logic.

   **Linux (Ubuntu 25.10):** Test A → reqforge.json=1000:1000,
   .collection.json=1000:1000, REQ-a.md=1000:1000 (host user
   1000:1000). Test B → REQ-b.md=104241:104241 (override
   4242:4242). The Test B host-side number is the container
   UID shifted by rootless Docker's user-namespace remap
   (~100000 offset); the override is being honoured inside
   the container, the host just sees the remapped view.
   Earlier the maintainer also sanity-checked the
   implementation via the `write::ownership` unit tests
   (worktree pointer resolution, overrides bypass,
   already-matching fast path).
   **macOS (26.4.1) / Docker Desktop:** Test A → host UID
   501:20 (mapped to 501:20 by virtiofs). Test B → host UID
   501:20 (override invisible on host side — expected).
   Note: on macOS the default port 36743 conflicted with a
   VS Code / Cursor extension-host listener on IPv4 localhost;
   the run used `REQFORGE_PORT=38743` to route around it.
   **Windows + Docker Desktop (Microsoft Windows 11 Pro):**
   Test A → .git=0:0, reqforge.json=0:0, .collection.json=0:0,
   REQ-a.md=0:0 (container view). Test B → REQ-b.md=4242:4242
   (override 4242:4242). Docker Desktop on Windows reports the
   bind mount's `.git` owner as root inside the container, so
   chown-from-`.git` correctly reproduces root ownership on
   written files — the spec-prescribed `REQFORGE_UID` override
   is the path a Windows operator who wants non-root ownership
   will use, and Test B confirms that path works end-to-end.

3. **Polling-based watcher edge cases** (Phase 2). Missed updates,
   duplicate events, rename detection. Plan the test cases before the
   watcher lands; don't wait for bugs in the wild.

   **Status (Phase 2):** implementation lives in
   `backend/reqforge-server/src/watcher.rs`; the test-case
   analysis below guided it.

   - **Missed updates within 1 s**: the fingerprint is
     `(path, mtime)`. mtime has ~1 s resolution on common
     filesystems (ext4, APFS), so two writes within a second that
     produce the same on-disk size _and_ land in the same mtime
     bucket would be missed. Mitigation: include file size in
     the fingerprint. Not done in Phase 2 — the real-world
     exposure is narrow (a rapid two-write sequence where neither
     goes through the ReqForge UI) and we'd rather not add cost
     to every tick. Revisit if a user reports missed updates.
   - **Duplicate events**: `state.publish()` is idempotent; the
     `Arc<World>` swap always fires a `ChangeEvent` but clients
     invalidate react-query, which debounces identical refetches.
     No consumer-visible duplication.
   - **Rename detection**: a rename shows up as one (path, mtime)
     pair vanishing and another appearing. Both changes trigger
     a refresh; rediscovery rebuilds the index by UUID, which is
     stable across the rename (per ART-moveRename), so incoming
     links keep resolving.
   - **File deletion**: removed paths drop out of the fingerprint;
     the watcher refreshes and discovery drops the artifact from
     the index.

   Four watcher-unit tests exercise fingerprint coverage
   (md/json only, ignore .txt), missing-prefix behaviour, mtime
   change detection, and file-add detection.

4. **Markdown editor feel** (Phase 2). Spike CodeMirror + react-markdown
   side-by-side in a tiny demo; make sure the keystroke-to-preview
   latency is acceptable at realistic artifact sizes before adopting
   the library.

   **Status (Phase 2):** passed against a ~20 kB Markdown
   document (varied grammar: headings, tables, nested lists,
   task list, blockquote, four language-tagged code fences, a
   long JSON block, footnote, inline HTML comment). Observed
   in a live session on 2026-04-19:

   - **Typing feel:** instant. No perceptible delay between
     keystroke and character appearing in the left pane.
   - **Preview catch-up:** fast enough that the maintainer
     could not discriminate a delay between stopping typing
     and the right pane catching up. Comfortably under 100 ms.
   - **Scrolling:** both panes scroll smoothly on their own.
     They scroll independently; linked scroll between editor
     and preview is a polish item, not required by
     UX-markdownEditor, deferred.
   - **Caret visibility (bug fixed in-session):** the default
     CodeMirror caret can render invisible against Tailwind's
     body colour. `MarkdownEditor` now ships a theme extension
     that pins `caretColor` and `.cm-focused .cm-cursor`
     border-colour to `currentColor`, with `drawSelection()`
     enabled for consistent selection rendering.

   Remediation levers (unused, recorded for future reference
   if a bigger artifact stresses things): debounced preview
   render, memoised ReactMarkdown tree, web-worker-offloaded
   render.

5. **Subprocess hygiene for the thumbnailer** (Phase 5). Phase 5
   introduces ReqForge's first subprocess invocations —
   LibreOffice-headless (`soffice --headless …`) and libvips /
   ImageMagick bindings. The failure mode to rule out early:
   orphaned / zombie subprocesses under container-uid mismatch,
   tempdir permission problems, stdio buffering stalls, and
   unbounded concurrency monopolising the runtime. Defer the
   check until sub-phase 5d lands (all the moving parts are in
   place by then); then run a live session against a mount
   whose host UID differs from the container UID, upload a docx,
   trigger a thumbnail, confirm no leftover `soffice` processes
   via `ps` inside the container, and walk the same sequence on
   each supported Docker host platform — Linux, macOS + Docker
   Desktop, Windows + Docker Desktop (mirrors Risk Check 2's
   platform matrix). Per-platform scripts in
   `scripts/risk-check-5/` so the maintainer doesn't have to
   retype the diagnostic steps on each host.

## Ongoing Concerns

These apply across phases rather than fitting a single one.

- **Tests with every phase.** Unit for domain logic, integration for
  storage + HTTP, end-to-end for UI flows. Tests ship with the phase
  that introduces the behaviour; retrofitting doesn't count.
- **Documentation kept current.** README recipes, `INTENTIONS.md`,
  and this roadmap are all updated as phases land. Stale docs in a
  project this documentation-heavy are worse than missing docs.
- **Logging and diagnostics.** Structured JSON logging is in from
  Phase 1. Verbosity dial via `REQFORGE_LOG_LEVEL`. `/healthz` and
  `/readyz` land in Phase 1; `/metrics` is optional across the
  project (per `TECH-observability`).
- **CI pipeline.** Expected to drive the existing `make` targets
  (`fmt-check`, `lint`, `test`, `docker-build`). Specific CI platform
  is an implementer choice; the ReqForge-side commitment is the
  Makefile surface.

## Phase Details

Phases 0 and 1a/1b/1c are detailed because we worked the task
breakdown out explicitly. Phases 2–11 have outcome-level descriptions
here; each will get a detailed task list on its own branch when the
work starts.

### Phase 0 — Scaffolding

**Outcome:** Repo has the skeleton everyone else depends on — build
chain, dev workflow, dev workspace, container image. Nothing
user-visible yet.

**Tasks:**

1. Top-level repo layout: `backend/` Cargo workspace root (initial
   crate `reqforge-server`); `frontend/` Vite project; `Dockerfile`
   at repo root; `docker-compose.yml` at repo root as reference.
2. `.reqforge-workspace/` directory with `example-system.json`,
   `example-docker-compose.yml`, and a `test-repos/sample/` fixture.
   `.reqforge-workspace/system.json`, `.reqforge-workspace/test-repos/`
   gitignored; only `example-*` files committed.
3. Dockerfile: multi-stage — Chainguard Rust builder → Chainguard
   Node builder → Wolfi runtime. Multi-arch amd64+arm64 build
   configuration.
4. `.gitignore` additions: `target/`, `frontend/dist/`,
   `frontend/node_modules/`, and the dev-workspace entries above.
5. Makefile extensions: `make dev` (run back-end and front-end dev
   servers, env vars pointing at `.reqforge-workspace/`),
   `make build`, `make test`, `make test-e2e`, `make docker-build`,
   `make docker-run`, `make docker-publish` (opt-in via env vars).
   `make fmt`, `make fmt-check`, `make lint` already exist and grow
   as new languages land.
6. README recipe sections for "run in dev" and "run in production,"
   each just pointing at the relevant `make` target.
7. Initial Cargo workspace, `reqforge-server` crate with a minimal
   `fn main` that prints a version banner. No HTTP yet.
8. Initial Vite + React + TypeScript project in `frontend/` with a
   landing page that says "ReqForge" and nothing else. No routing
   yet.

### Phase 1a — Read-only backend core

**Outcome:** Rust back-end discovers bind-mounted repositories,
loads their content-hosted artifacts into memory, builds the UUID
index, and exposes read-only HTTP endpoints. No UI yet on this
branch.

**Tasks:**

1. **Serde structs** for every file type ReqForge reads: project
   config (`reqforge.json`), collection config (`.collection.json`),
   System config, artifact metadata (content / blob / URL shapes),
   link payload, review-log entry, TODO. Include `schemaVersion`
   handling and `legacy` / overflow-bucket support (per
   `FORMAT-fieldTolerance`).
2. **Mount discovery:** scan `REQFORGE_MOUNT_PREFIX` (default
   `/repos`); classify each subdir as Project / Needs-init / No-git /
   Read-only per `DEPLOY-mountValidityStates`.
3. **Project loader:** parse `reqforge.json`, walk `artifacts/` (or
   `artifactsPath` override), load `.collection.json` in each
   Collection directory, load each `.md` artifact (content-hosted).
   Blob and URL artifacts: struct defined, loader stubbed with a
   TODO for Phase 5.
4. **Frontmatter parser:** split `---`-delimited JSON frontmatter
   from the Markdown body. Strict JSON parse; on failure, report per-
   artifact error without poisoning the Project load.
5. **UUID index:** `HashMap<Uuid, ArtifactLocation>` built at load
   time; lookup API for resolving link targets and external queries.
6. **System config loader (optional):** read
   `REQFORGE_SYSTEM_CONFIG` if set. When unset, run in unnamed-system
   mode. Surface expected-but-missing projects per
   `DEPLOY-systemConfigFile`.
7. **HTTP server (axum):** endpoints `GET /healthz`, `GET /readyz`
   (200 once discovery and indexes are built, 503 during), `GET
/api/projects`, `GET /api/projects/:slug`, `GET
/api/projects/:slug/collections`, `GET
/api/projects/:slug/collections/:prefix`, `GET
/api/projects/:slug/collections/:prefix/artifacts`, `GET
/api/artifacts/:uuid`. All read-only. All JSON responses.
8. **SSE endpoint stub** at `/api/events`: connection establishes,
   stream stays open, no events emitted yet. Actual event pushing
   waits for the polling watcher in Phase 2.
9. **CORS** for dev: allow the Vite dev server's origin.
10. **Structured JSON logging** via `tracing-subscriber`, respecting
    `REQFORGE_LOG_LEVEL`.
11. **Back-end tests:** serde round-trip tests for every schema,
    loader tests against `.reqforge-workspace/test-repos/sample/`,
    HTTP endpoint tests with axum's test harness.

### Phase 1b — Read-only React frontend

**Outcome:** Browser shows the System Home, drills into Projects,
Collections, and individual artifacts, rendering their Markdown
bodies. Uses the Phase 1a HTTP API.

**Tasks:**

1. **App shell:** layout with a header (ReqForge branding + port
   indicator in dev), a sidebar (Project and Collection navigation),
   and a main content area.
2. **Routing (react-router):** `/` (System Home), `/projects/:slug`,
   `/projects/:slug/collections/:prefix`, `/projects/:slug/collections/:prefix/artifacts/:name`
   (or `/artifacts/:uuid` for direct UUID access).
3. **API client:** typed fetch wrappers; react-query hooks per
   endpoint with standard loading / error / success states.
4. **System Home view:** list mounted repos with validity-state
   badges (Project / Needs-init / No-git / Read-only). Empty state
   per `UX-emptyStates` when no repos are mounted.
5. **Project view:** list Collections. Empty state when Project has
   no Collections.
6. **Collection view:** virtualised list of artifacts (TanStack
   Virtual, even though Phase 1 workloads are small — establishes
   the pattern). Empty state when Collection has no artifacts.
7. **Artifact detail view:** render Markdown body via react-markdown
   - remark-gfm + rehype-highlight. Show title, tags, link counts
     (incoming / outgoing, values cached), and review-state summary.
8. **Navigation:** breadcrumbs (System / Project / Collection /
   Artifact). Sidebar reflects current location.
9. **Basic Tailwind styling:** consistent typography, comfortable
   reading width in the artifact view, sensible spacing. Nothing
   elaborate; enough to not look unfinished.
10. **Front-end tests:** component tests for each view via vitest +
    React Testing Library; API-client hook tests with mocked fetch.

### Phase 1c — End-to-end smoke testing

**Outcome:** Automated smoke suite against the selenium-chrome
container validates the full vertical slice from Phase 1a + 1b. Run
via `make test-e2e`.

**Tasks:**

1. **Smoke harness** in `frontend/tests/e2e/`, using
   `selenium-webdriver`. Target: the external selenium-chrome
   container's WebDriver endpoint.
2. **Fixture:** use `.reqforge-workspace/test-repos/sample/` (created
   in Phase 0) or a dedicated e2e-fixture repo with a known Project,
   Collection, and artifact structure.
3. **Smoke scenarios:**
   - Load `http://localhost:36743`, confirm System Home renders the
     sample Project with the correct validity-state badge.
   - Click into the Project, confirm Collections render.
   - Click into a Collection, confirm the artifact list renders.
   - Click into an artifact, confirm title and Markdown body appear
     as rendered HTML (not raw text).
4. **`make test-e2e` target:** runs the smoke suite; assumes the
   ReqForge back-end+front-end are running (either via `make dev` in
   another terminal or a dedicated `make test-e2e-with-server` helper
   that spins everything up, runs tests, tears down).
5. **First risk-adjusted check runs here too:** verify JSON
   frontmatter rendering on both GitHub and GitLab (push a sample
   `.md` to a test repo on each, confirm the frontmatter renders as
   a YAML-ish table and the body below is normal Markdown). Document
   the result in the Work Item; revisit the delimiter decision if
   either platform breaks.

### Phase 2 — Artifact CRUD with Markdown editor

**Outcome:** Full create / update / delete on content-hosted
artifacts from the UI. CodeMirror-based editor with live preview.
Project init wizard and Collection wizards usable. UID/GID matching
on write (per `DEPLOY-chownFromDotGit`). Atomic writes (per
`STOR-atomicWrites`). Polling filesystem watcher (per
`DEPLOY-pollingWatch`) with the three-option conflict prompt
(per `UX-externalEditConflict`). Second and third risk-adjusted
checks run here (chown-from-`.git`, polling-watcher edge cases).

### Phase 3 — Typed link authoring

**Outcome:** Link CRUD with the type-ahead picker (per
`UX-linkCreationPicker`). Link storage in frontmatter per
`FORMAT-linkPayloadSchema`. Cross-repo resolution via the UUID
index. Unresolved-link display per `TRACE-unresolvedLinks`. The
`active`, `derived`, `tags`, `outlineLevel` fields surface in the
UI per their respective ART requirements.

Split into three commit-sized sub-phases on the single branch:
**3a** backend catalog + resolution + search (no UI), **3b** link
CRUD API + picker UI + OutgoingLinks rewrite, **3c** surrounding-
fields polish + unresolved-mount affordance + docs. Each sub-phase
stands on its own — 3a ships server-side without breaking the
existing UI; 3b plugs the authoring surface in; 3c closes the
outstanding ART-field gaps.

**Tasks:**

1. **Built-in link-type catalog** (3a). Hard-code the seven
   built-in types from `TRACE-linkCatalog` —
   `derives-from`, `satisfies`, `verifies`, `supersedes`,
   `cites`, `conflicts-with`, `related-to` — in
   `backend/reqforge-server/src/links/catalog.rs`. Each entry
   carries forward name, inverse name, `directed`, `acyclic`.
   Hard-coded rather than loaded from a JSON fixture: built-ins
   are spec, not data, and extensibility already lives in the
   System config.
2. **Effective catalog resolver** (3a). `effective_catalog(system)`
   returns built-ins plus System-declared types (per
   `TRACE-linkExtensibility`), with built-ins winning on
   name collisions. Resolved list is cached on `World` so every
   handler reads one view.
3. **Link-type endpoint** (3a). `GET /api/link-types` returns the
   effective catalog as `LinkTypeDto` with a `source` field
   distinguishing `"builtin"` from `"system"`. Read-only,
   regenerated on each `World` publish.
4. **Unresolved-link surfacing** (3a). `ArtifactDetail.links`
   changes shape from raw `Link` to `LinkView { targetUuid, type,
hint, resolution: "resolved" | "unresolved" | "unknownType",
typeMetadata, targetSummary }`. Server computes `resolution`
   per-request against the effective catalog + UUID index, keeping
   client-side resolution logic out of scope per
   `TRACE-unresolvedLinks`. The raw `Link` serde stays intact for
   on-disk IO.
5. **Cross-repo incoming-links regression test** (3a). The
   existing `list_incoming_links` already walks every loaded
   project; add a two-project integration test confirming a link
   from project A to project B surfaces on B's incoming-links
   response. Covers `TRACE-crossRepoLinks`.
6. **Link-search endpoint** (3a). `GET /api/artifacts/search?q=…
&limit=25&exclude=<uuid>` does a case-insensitive
   substring match across all loaded projects with an
   exact-prefix match boost, returning `{ uuid, projectSlug,
collectionPrefix, artifactName, title, active }`. Linear
   scan is fine for now — Tantivy lands in Phase 7c.
7. **Link write validation** (3b). Extend
   `UpdateArtifactRequest` with an optional `links` field
   (absent = unchanged; present = full-array replacement, matching
   `tags` / `description` semantics). Validator rejects empty
   `targetUuid`, empty `type`, unknown type, and self-links;
   unresolved targets are allowed (cross-repo authoring must work
   without the target repo mounted).
8. **Hint auto-population on write** (3b). When the UUID is
   resolvable at write time, the server overrides the
   client-supplied hint with the authoritative one from the UUID
   index; unresolved writes preserve the client hint. Lazy-hint
   convention is consistent with `ART-moveRename`.
9. **Frontend API bindings** (3b). Extend `api/types.ts` with
   `LinkType`, `LinkResolution`, `LinkView`, `ArtifactSearchResult`;
   update `ArtifactDetail.links` type. New `linkTypes()` and
   `searchArtifacts()` in `api/client.ts`. Hooks `useLinkTypes()`
   and `useArtifactSearch(q, exclude)` in `api/queries.ts`; search
   hook gates on `q.length >= 1` with a 150ms debounce at the call
   site. New `useUpdateArtifactLinks(uuid)` specialisation.
10. **Link-type badge component** (3b). Small coloured chip with
    a tooltip showing directedness and inverse name; reused in
    `OutgoingLinks` and the picker.
11. **Rewrite `OutgoingLinks.tsx`** (3b) to consume `LinkView[]`.
    Groups by type. `"resolved"` links through to the target;
    `"unresolved"` shows the hint plus a muted
    "unresolved — mount `<slug>`" affordance; `"unknownType"`
    renders with an
    "unknown link type" indicator (per `TRACE-linkExtensibility`).
    Read-mode has no per-link delete — users must enter edit mode
    first (deliberate, small-UX-surface choice).
12. **Link picker component** (3b). Two-step flow per
    `UX-linkCreationPicker`: link-type select (Radix `Select`),
    then a type-ahead target picker (cmdk-backed combobox) wired
    to `useArtifactSearch`. Results show
    `projectSlug/collectionPrefix/artifactName` + title, with the
    current artifact's own project ranked first. Escape cancels;
    Enter commits.
13. **Add-link integration in `ArtifactEditor`** (3b). "Add link"
    button opens the picker in a Radix `Dialog`; commits append to
    local staged state and flush through the existing save (same
    optimistic+conflict-modal flow as Phase 2). Delete-in-edit-
    mode removes from staged state; no separate PATCH endpoint
    per the full-array-replacement decision.
14. **`outlineLevel` field UI** (3c). Read-mode: muted pill next
    to the title in `ArtifactView` (e.g., `"1.2.3"`). Edit-mode:
    free-text input. Included in the update payload.
15. **`derived` field edit control** (3c). Already displayed in
    read mode; add a checkbox in `ArtifactEditor` and thread through
    the update payload.
16. **`tags` chip-rendering pass** (3c). `TagList.tsx` renders
    read-mode tags as chips and handles the empty case. No
    implicit linking from tags (explicit per `ART-tagsField`).
17. **Unresolved-mount affordance** (3c).
    `UnresolvedBadge.tsx` renders the
    "unresolved — mount `<slug>`" copy; when the hinted slug
    matches a currently-mounted-
    but-`NeedsInit` directory, the badge offers a shortcut to the
    System Home with that mount highlighted. Pure client side off
    `LinkView.resolution` + existing `useMounts()`.
18. **Backend integration tests** (3a + 3b).
    `tests/typed_links.rs` covers: create → add link → GET shows
    resolved; link to unmounted UUID → `"unresolved"` with hint
    preserved; System-declared type → resolved with `source:
"system"`; unknown type → `"unknownType"`; two-project
    incoming-links round-trip; catalog-endpoint includes built-ins
    with zero System-declared types; built-in-versus-System name
    collision keeps the built-in; search endpoint handles empty
    query, short query, substring match, prefix-boost, self-
    exclude.
19. **Frontend unit tests** (3b + 3c). New tests for
    `OutgoingLinks` (three resolution states + in-edit remove),
    `LinkPicker` (type-then-target flow, keyboard nav, debounce,
    self-exclude), `LinkTypeBadge`, `UnresolvedBadge`, and an
    `ArtifactEditor` extension covering staged link add/remove +
    save round-trip.
20. **End-to-end smoke test extension** (3b). One browser-driven
    happy path: open artifact → Add link → pick type → type-ahead
    find target → save → reload → link appears in
    `OutgoingLinks` with the right type and a live resolved
    target. Small deliberately — backend covers the
    permutations.
21. **Fixture augmentation** (3c). Add at least a second
    collection with one linkable artifact to
    `.reqforge-workspace/example-test-repos/sample-project/`
    if the existing fixture doesn't already satisfy the E2E
    scenario. Optionally a second repo for cross-repo unresolved-
    link coverage.
22. **Coverage pass + ROADMAP Phase 3 "passed" note** (3c). Rerun
    `make test` (backend + frontend) + `make fmt-check` +
    `make lint`. Confirm line coverage stays ≥80%. Update
    this Phase 3 block once sub-phases land.

**Status (Phase 3):** Shipped. Backend 144 tests / frontend 60
tests green after 3a / 3b / 3c; line coverage at 84.73 % and
function coverage at 83.49 % (up from 83 / 81). Hard-coded
built-in catalog + System-declared extras landed in 3a; link
write validation, the two-step picker, and the three-state
`OutgoingLinks` rewrite in 3b; outline-level / derived edit
controls, chip-style tag rendering, and the
`UnresolvedBadge` with "init this mount" shortcut when a
`NeedsInit` mount matches the hint slug in 3c. No new risk
checks needed in Phase 3.

### Phase 4 — Review workflow

**Outcome:** Review log, review pane with since-last-approval
section and TODO timeline (per `UX-reviewPane`), review queue (per
`UX-reviewQueue`), approve / reject / add-resolve-TODO / re-request
actions (per `UX-reviewActions`). Weak reviewer identity dropdown
(per `REVIEW-reviewerIdentity`). End of Phase 4 is the **dogfooding
milestone** — ReqForge can now manage its own requirements. The
`requirements-doorstop/` bootstrap can be hand-migrated or left in
place until Phase 8's doorstop importer lands.

Split into four commit-sized sub-phases on the single branch:
**4a** backend state derivation + reviewer-identity plumbing (no
UI change); **4b** review-action write endpoint + review-queue
endpoint; **4c** review pane + reviewer dropdown + queue page;
**4d** dogfooding walk-through, fixtures, e2e, coverage pass.
Cutting 4b and 4c apart keeps the backend API stable before the
frontend is built against it, same trade Phase 3 made.

**Design decisions locked in before coding starts:**

- **Action/entry mapping is 1:1.** One HTTP review call = one
  review-log entry. A `reject-with-TODOs` action produces a
  single `rejected` entry with `addedTodos` populated; it does
  not also emit `todo-added` entries. For now the UI offers
  exactly one TODO per rejection; multiple-TODOs-per-rejection
  is a deferred feature (the serde schema already carries
  `addedTodos: Vec<…>` so forward compatibility is free).
- **Reviewers-file path.** A new `REQFORGE_WORKSPACE_DIR` env
  var points at the workspace directory (defaulting to
  `$HOME/.reqforge-workspace` in production; `make dev` sets
  it to `<repo>/.reqforge-workspace`). `reviewers.json` lives
  at `<workspace>/reviewers.json`. Mirrors the existing
  `REQFORGE_MOUNT_PREFIX` pattern.
- **"Since last approval" diff source.** Snapshot the artifact
  body and metadata under `.reqforge-workspace/review-snapshots/
<uuid>/<timestamp>/` on every approve action and diff current
  against the snapshot. Upgrade to a gitoxide-backed
  historical-diff when `UX-diffView` lands in Phase 5.
- **Review actions bypass `PUT /api/artifacts/:uuid`.** All five
  actions go through a new `POST /api/artifacts/:uuid/reviews`
  so the validator sees the prior state (e.g. to reject an
  approve when blocking TODOs are open). `reviewLog` stays
  immutable from the generic `UpdateArtifactRequest`, as it
  already is per the DTO comment.
- **Review queue is server-rendered.** `GET /api/reviews/queue`
  returns both sections with filter params applied server-side,
  not per-artifact client fan-out — same decision 3a made for
  `/api/artifacts/search`.
- **Single-reviewer now, N-of-M future.** The state-derivation
  function consumes `&[ReviewLogEntry]` as an opaque event
  stream and returns a single derived state. When N-of-M lands
  (`REVIEW-futureMultiReviewer`), only the derivation function
  changes; the schema stays intact. A comment on the state
  module documents this invariant.
- **`re-request-review` is a log-only signal.** It sets the
  derived state to `ReRequested` (distinct from
  `NeverReviewed`) without resetting history, and the queue
  shows these under "Awaiting review" sorted by the re-request
  timestamp.

**Tasks:**

1. **Review-state types** (4a) in
   `backend/reqforge-server/src/reviews/state.rs`. Introduce
   `ReviewState` (`NeverReviewed` / `Approved` / `Rejected` /
   `ReRequested`) and `DerivedReviewState { state,
last_approval_at, blocking_todos, last_event_at,
last_reviewer }`. Pure functions over `&[ReviewLogEntry]`;
   no IO. Comment records the N-of-M-friendly event-stream
   contract (`REVIEW-futureMultiReviewer`).
2. **Blocking-TODO algorithm** (4a). A TODO's state is the fold
   of later `todo-resolved` entries against its id; an
   artifact's open TODOs are the still-open set accumulated
   from any `rejected` entry newer than the most recent
   `approved` entry (`REVIEW-blockingTodos`).
3. **Reviewer-identity parser** (4a) in
   `backend/reqforge-server/src/reviews/identity.rs`. Pure
   parser for `.git/config` `[user] name = …` (no git binary,
   no `gix`; plain INI). Separate function reads
   `<workspace>/reviewers.json` (absent file → empty list,
   malformed → typed error). `ReviewerIdentityOptions`
   aggregates `{ git_default, persisted, session }`.
4. **Session-identity cache** (4a) on `AppState`. An in-memory
   list of reviewers submitted this container lifetime, kept
   separate from the persisted file. Deduplicate
   case-sensitively; snapshots are cloned.
5. **`ReviewState` DTO in `ArtifactDetail`** (4a) in
   `src/http/dto.rs`. Add `review_state:
DerivedReviewStateDto` alongside the preserved raw
   `review_log`. Computed server-side, mirroring Phase 3a's
   `LinkView` pattern.
6. **`GET /api/reviewers`** (4a). Returns
   `{ gitDefault, persisted, session }`. Accepts optional
   `?projectSlug=` so the git-config default is read from the
   target mount's `.git/config` rather than from the
   workspace.
7. **`REQFORGE_WORKSPACE_DIR` plumbing** (4a). New env var
   resolved in `src/main.rs` / `DiscoveryConfig`, defaulting to
   `$HOME/.reqforge-workspace`. `make dev` sets it to the
   repo-local `.reqforge-workspace/`. Wire through to the
   identity loader and (later) to the review-snapshots
   directory.
8. **Mount-level git user resolution** (4a). Expose
   `LoadedProject::git_user_name()` so the reviewer endpoint
   and any per-artifact default-reviewer lookup picks up the
   right mount's `.git/config`, not the workspace's.
9. **Review-action write validator** (4b) in
   `backend/reqforge-server/src/reviews/validate.rs`. Given
   `(artifact, action)`: `approve` fails when any blocking
   TODO is open; `resolve-todo` fails when the id isn't in the
   open set; `re-request-review` requires current state to be
   `Approved` or `Rejected`; reviewer string non-empty; `add-todo`
   allowed in any state.
10. **`POST /api/artifacts/:uuid/reviews`** (4b). Request DTO
    `CreateReviewRequest { reviewer, action, explanation?,
addedTodos?, resolvedTodoIds? }`. Handler clones the
    current artifact, runs the validator, appends a single new
    entry to `review_log`, writes via the existing atomic
    `write_artifact_file` path, refreshes the world, records
    the session identity, returns the updated
    `ArtifactDetail`.
11. **Reviewer persistence side effect** (4b). After a
    successful review write, if the reviewer isn't in
    `<workspace>/reviewers.json`, atomically rewrite the file
    with the new entry appended. Reuses
    `write::atomic_write`.
12. **Approval snapshot side effect** (4b). When an `approve`
    action succeeds, write the current artifact body +
    metadata to
    `.reqforge-workspace/review-snapshots/<uuid>/<timestamp>/`
    before returning. Prunes snapshots older than the tail of
    any artifact's "since last approval" window so the
    directory doesn't grow unboundedly.
13. **`GET /api/reviews/queue`** (4b). Returns `{
awaitingReview, blockingTodos }` where each item is `{
uuid, projectSlug, collectionPrefix, artifactName, title,
state, lastEventAt, modifiedAt, blockingTodoCount }`.
    Default order: awaiting-review oldest-modification-first,
    blocking-TODOs newest-rejection-first. Query params:
    `projectSlug`, `collectionPrefix`, `shape`, `tag`,
    `reviewer`, `order`. Secondary sort by slug → prefix →
    name to keep ties stable across watcher rescans.
14. **Listing review-state hint** (4b). Add `reviewState:
ReviewState` to `ArtifactListing` so the collection view
    can badge at-a-glance. Reuses the derivation from task 1.
15. **Frontend API types + queries** (4c). `ReviewState`,
    `DerivedReviewState`, `BlockingTodo`, `QueueEntry`,
    `ReviewerIdentityOptions`, `ReviewActionInput`.
    `useReviewers(projectSlug)`, `useReviewQueue(filters)`,
    `useSubmitReview(uuid)`. Submit mutation invalidates the
    artifact, queue, and reviewers caches.
16. **`ReviewerSelect`** (4c). Dropdown with three groupings:
    git default, session, persisted. Default selection is the
    git default; most-recent-in-session bumps to the top on
    next open. Escape-hatch free-text input for typing a new
    identity.
17. **`ReviewPane`** (4c). Replaces the minimal
    `ReviewSummary` on `ArtifactView.tsx`. Subsections: current
    state badge, unresolved blocking TODOs (with per-TODO
    resolve action), review-log list, and the collapsible
    "Since last approval" section.
18. **`SinceLastApprovalDiff`** (4c). Content diff between
    body-at-last-approval and current body plus a metadata
    diff table (title, description, tags, links, outlineLevel).
    Uses `diff` (Kpdecker) for line-based diffing; panel is
    suppressed entirely when the artifact has never been
    approved (per `UX-reviewPane`'s banner rule).
19. **`SinceLastApprovalTimeline`** (4c). Chronological log
    from last-approval to now. Resolved TODOs render with
    strike-through; unresolved TODOs render with a pending
    indicator.
20. **Review-action dialogs** (4c) — `ApproveDialog`,
    `RejectDialog` (single TODO input for now, multiple
    deferred), `AddTodoDialog`, `ReRequestDialog`. Each is
    gated on a valid `ReviewerSelect` value. Approve disables
    submit when any blocking TODO is open, with an inline
    explanation.
21. **Resolve-TODO inline action** (4c). Per-TODO button
    dispatches `useSubmitReview` with `action:
"resolve-todo"` and the TODO id, inside a compact
    `ReviewerSelect` popover.
22. **`ReviewQueuePage`** (4c) routed at `/reviews`.
    `@tanstack/react-virtual` for both sections. Filter bar
    with project / collection / shape / tag / reviewer
    selectors and an ordering toggle. Empty state renders when
    both sections are empty.
23. **Sidebar queue badge** (4c) in `layout/Sidebar.tsx`. Count
    is `awaitingReview.length + blockingTodos.length`.
    Invalidates alongside the queue query.
24. **`ArtifactView` rewire** (4c). Swap
    `<ReviewSummary>` for `<ReviewPane>`; keep
    `ReviewSummary` exported if anything else consumes it, or
    delete it if not.
25. **Fixture repo update** (4d). Add one artifact in each of
    the four derived states to
    `.reqforge-workspace/example-test-repos/sample-project/`
    plus an `example-reviewers.json` template committed next
    to the existing `example-*` files (real `reviewers.json`
    is gitignored per `DEPLOY-devWorkspace`).
26. **E2E happy path** (4d) in
    `frontend/tests/e2e/review-workflow.spec.ts`. One
    scenario: approve a never-reviewed artifact → reject
    another with a TODO → resolve the TODO → re-request
    review → approve. Covers the queue-page transitions and
    the sidebar badge.
27. **Dogfooding walk-through** (4d). Run `make dev` against
    the real `requirements-doorstop/` content (via a hand
    conversion if needed), drive at least one REVIEW-\*
    requirement through the full workflow end-to-end, and
    record findings in this Phase 4 block.
28. **Coverage pass + ROADMAP Phase 4 "Shipped" note** (4d).
    Rerun `make test` + `make fmt-check` + `make lint` and
    confirm line coverage stays ≥ 80 % (target: match or
    exceed Phase 3's 84.73 %). Update this block with the
    final shipped note per the Phase 3 template.

**Status (Phase 4):** Shipped (minus the in-browser dogfooding
walk-through, pending user action). Backend 204 tests / frontend
73 tests green after 4a / 4b / 4c; line coverage 84.43 % and
function coverage 83.48 % (even with Phase 3's 84.73 / 83.49).
4a introduced the `reviews::state` derivation, the
`.git/config` INI parser, the session-identity cache on
`AppState`, the `REQFORGE_WORKSPACE_DIR` env var, the
`reviewState` DTO on `ArtifactDetail`, and `GET /api/reviewers`.
4b added the validator, `POST /api/artifacts/:uuid/reviews`,
reviewer persistence, approval-time artifact snapshots, the
`GET /api/reviews/queue` endpoint (with server-side filters),
and the `reviewState` hint on `ArtifactListing`. 4c wired the
UI: `ReviewerSelect`, four review-action dialogs plus the
inline resolve-TODO popover, `ReviewPane` (replacing the
stopgap `ReviewSummary`), the "since last approval" diff +
activity-timeline sub-panels, `ReviewQueuePage` at `/reviews`,
and the sidebar badge. 4d augmented the sample fixture with
one artifact in each of the four derived states
(`REQ-helloWorld` never-reviewed, `REQ-greeting` approved,
`REQ-rejected` rejected-with-blocking-TODO,
`REQ-rerequested` approved-then-re-requested), added an
`example-reviewers.json` template, and shipped a
selenium-gated end-to-end smoke test driving the full
approve → reject → resolve → re-request → approve flow. No
new risk-adjusted checks.

Outstanding user-side work: bring up `make dev` against the
live `requirements-doorstop/` content, drive one REVIEW-\*
requirement through the full review workflow end-to-end, and
append findings under this block. That's the dogfooding gate
itself — it has to be done in a real browser against the
live tree, so the repo's automated suite can't cover it.

### Phase 5 — Non-content artifact shapes

**Outcome:** Blob upload with sidecar, URL artifact creation, lazy
thumbnail generation with on-disk cache (per `UX-uploadPreview`),
shape-aware diff view (per `UX-diffView`, using gitoxide for
historical access), URL-artifact checking action (per
`UX-urlArtifactChecking`).

Split into four commit-sized sub-phases on the single branch:
**5a** loaders plus sidecar IO plus shape-aware write path (no
new endpoints); **5b** upload plus URL CRUD plus URL-check action
plus blob-download endpoint; **5c** thumbnail pipeline plus
on-disk cache plus blob/URL UI plus three-tab new-artifact
dialog; **5d** gitoxide-
backed diff view (replacing Phase 4c's LCS), shape-aware diff
renderer, Dockerfile additions, fixture augmentation, e2e, and
Risk Check 5. Each sub-phase stands on its own — 5a lands
invisibly, 5b exposes a usable backend via `curl`, 5c is the
first user-visible surface, 5d is the diff + gitoxide + image
upgrade.

**Design decisions locked in before coding starts:**

- **Sidecar path layout.** Flat sibling per
  `FORMAT-blobSidecar`: a blob artifact `DES-spec.pdf` sits next
  to `DES-spec.pdf.reqforge.json`. URL artifacts are a single
  `<name>.reqforge.json` with no peer binary.
- **Thumbnail cache.**
  `<workspace>/thumbnail-cache/<first-two-hex>/<content-hash>/
512.png`, sharded to keep any one directory bounded. LRU-
  evicted when the cache exceeds
  `REQFORGE_THUMBNAIL_CACHE_MAX_BYTES` (default 500 MB).
- **Large-blob cap.** Enforced via
  `REQFORGE_MAX_BLOB_BYTES`. Default 50 MiB — deliberately on
  the low side because we don't have data yet on the realistic
  upper bound; the env var is the primary knob and the 413
  error message tells operators exactly how to raise it.
- **URL-check method.** HEAD first, fall through to GET on
  405 / 501 / connection-reset-after-HEAD. 10-second timeout
  (configurable via `REQFORGE_URL_CHECK_TIMEOUT_SECS`). No
  retries. Up to 10 redirects.
- **Image diff for blobs.** Side-by-side preview only for
  Phase 5 — no pixel-level / XOR-overlay image-diff. Recorded
  as a deferred follow-up for a later phase if the tool ever
  benefits from visual diff overlays.
- **gitoxide repo caching.** Lazy-open per mount on first
  history request, cached in a process-wide
  `DashMap<PathBuf, gix::Repository>` on `AppState`; evicted
  when a mount disappears on `refresh()`.
- **LibreOffice in the shipped image.** Hard dependency in the
  runtime Dockerfile (LibreOffice-headless + libvips +
  ImageMagick). Provider code soft-fails when the binary is
  missing so dev-machine `cargo run` still boots without the
  tooling installed.
- **URL edits.** `PUT /api/artifacts/:uuid` gains an optional
  `url` field; the validator rejects it on non-URL shapes. No
  dedicated URL endpoint.
- **Thumbnail concurrency.** `spawn_blocking` with a global
  semaphore of 2 concurrent generators. Duplicate in-flight
  requests for the same `<content-hash>` coalesce via a
  `tokio::sync::OnceCell` so two viewers loading the same docx
  don't spawn two `soffice` processes.
- **Diff fallback.** When the gitoxide-backed history endpoint
  can't resolve the last-approval commit (shallow clone,
  non-git mount in the future, etc.), fall back to Phase 4b's
  approval-snapshot diff and show a banner: "Historical git
  context unavailable; diff computed against last approval
  snapshot instead."
- **MIME sniffing on upload.** Use `infer` for a lightweight
  magic-byte check on every upload; reject uploads whose magic
  bytes don't match the claimed extension with a 400.

**Tasks:**

1. **Sidecar path helpers** (5a) in
   `backend/reqforge-server/src/schema/sidecar.rs`. Pure
   functions: `sidecar_path_for_blob`,
   `blob_path_for_sidecar`, `url_artifact_filename`. No IO.
2. **Blob sidecar loader** (5a) in `src/load/blob.rs`. Reads
   the JSON sidecar, verifies the binary peer, stats size +
   mtime + sha256 hash (thumbnail-cache keying). Extends
   `LoadedArtifact` with `blob: Option<BlobFacts>`. Rejects
   shape-mismatched sidecars and paths that escape the project
   root.
3. **URL artifact loader** (5a) in `src/load/url.rs`. Reads
   `.reqforge.json`, validates `shape == "url"`, enforces
   HTTP(S) URL scheme.
4. **Collection walker extension** (5a) in `src/load/project.rs`.
   Walk `.reqforge.json` entries alongside `.md`; dispatch on
   parsed `shape`. Orphan-binary / orphan-sidecar diagnostics
   rather than hard errors, so a partial upload doesn't
   poison discovery.
5. **Shape-specific DTO fields** (5a) in `src/http/dto.rs`. Add
   `BlobDetailDto { byteSize, contentHash, mediaType,
downloadUrl }` and surface `url`, `checkedAt`, `checkStatus`
   on `ArtifactDetail` for URL artifacts. `downloadUrl` points
   at the 5b blob-stream endpoint.
6. **Extension allowlist** (5a). Enumerate supported blob
   extensions and reject sidecars pointing outside the list
   (defensive against a hand-authored sidecar pointing at
   `/etc/passwd`-flavoured paths).
7. **Shape-aware write path** (5a) in `src/write/sidecar.rs`.
   Factor `write_sidecar_only` and `write_blob_and_sidecar` out
   of the content-hosted write path; both reuse
   `atomic_write` + `reconcile_ownership`.
8. **`LoadedProject::git_repo_path()`** (5a). Surfaces
   `<root>/.git` when present. Consumed by 5d's gitoxide
   handle. No gitoxide dep yet.
9. **`POST /api/projects/:slug/collections/:prefix/artifacts/blob`**
   (5b). Multipart handler accepting `file` + metadata fields.
   Writes the binary and the sidecar siblings. Validates
   extension, rejects duplicate names, enforces the
   `REQFORGE_MAX_BLOB_BYTES` cap via axum's body-limit
   extractor.
10. **`PUT /api/artifacts/:uuid/blob`** (5b). Re-upload
    replaces the binary, preserves UUID / review log / links
    per `ART-uploadReplaceOnly`. Extension-change handled
    atomically (write new, rename, delete old); metadata
    `modifiedAt` bumps.
11. **`POST /api/projects/:slug/collections/:prefix/artifacts/url`**
    (5b). JSON handler; validates URL scheme via `url::Url::parse`.
12. **`PUT /api/artifacts/:uuid`** (5b) extended with
    `url: Option<String>`. Validator rejects the field on
    non-URL shapes.
13. **`POST /api/artifacts/:uuid/check-url`** (5b) in
    `src/urls/check.rs`. Runs one HEAD (fall through to GET
    per the locked decision), classifies outcome into a
    bounded status set, persists `checkedAt` and `checkStatus`
    via the 5a write path. Uses `reqwest` with
    `rustls-tls-webpki-roots` only.
14. **`POST /api/collections/:slug/:prefix/check-urls`** (5b)
    bulk action. Sequential with a concurrency cap of 4; per-
    entry failures don't abort the batch.
15. **Large-upload guard** (5b) via
    `axum::extract::DefaultBodyLimit` on the upload routes;
    413 response points operators at URL-reference artifacts
    and at `REQFORGE_MAX_BLOB_BYTES`.
16. **`GET /api/artifacts/:uuid/blob`** (5b). Streams the
    binary with `Content-Type` from the sidecar's
    `mediaType`, `ETag` = content hash, `Content-Disposition:
inline`.
17. **Route wiring** (5b). Register the new 5b routes in
    `http/mod.rs`; apply the body-limit middleware to uploads
    only.
18. **Thumbnail provider trait** (5c) in
    `src/thumbnails/mod.rs`. `ThumbnailProvider` + a
    registry that walks a static list and returns the first
    provider accepting a given media type.
19. **LibreOffice provider** (5c) in `thumbnails/libreoffice.rs`.
    Shells out to `soffice --headless --convert-to pdf`, pipes
    the first page through the image provider. 30-second
    timeout, stderr captured. Feature-detected at startup; if
    `soffice --version` fails, the provider isn't registered
    and docx/xlsx/pptx fall through to the "no thumbnailer"
    tier.
20. **libvips image provider** (5c) in
    `thumbnails/libvips.rs`. Handles PNG / JPG / GIF / PDF-
    first-page / SVG. 512 px longest-edge PNG output.
    ImageMagick is the fallback for formats libvips rejects.
21. **Thumbnail cache** (5c) in `thumbnails/cache.rs`.
    Hash-sharded layout, size-cap LRU eviction, content-hash
    keyed (naturally invalidating on re-upload since the hash
    changes).
22. **`GET /api/artifacts/:uuid/thumbnail`** (5c). Cache-hit
    path returns the PNG directly; cache-miss runs the
    provider in `spawn_blocking` under the global-semaphore-
    of-2 guard and the `OnceCell` coalesce. 404 with
    `{ reason: "no-thumbnailer-for-format" }` when no provider
    matches.
23. **Frontend shape types** (5c) in `api/types.ts`.
    `BlobDetail`, extend `ArtifactDetail`, new request shapes
    for blob / URL creation, URL check, bulk URL check.
24. **Frontend client + hooks** (5c) in `api/client.ts` +
    `api/queries.ts`. Multipart bodies via `FormData`. Query
    invalidation after every mutation.
25. **`BlobArtifactView.tsx`** (5c). Three rendering tiers:
    browser-native inline render (PDF, images, text),
    thumbnail + download link (Office formats), icon + size +
    download fallback (unsupported). Replace-file button
    opens a dialog wired to `PUT /api/artifacts/:uuid/blob`.
26. **`UrlArtifactView.tsx`** (5c). External link with a
    colour-coded status pill (`ok` green, server-error / timeout
    amber, `not-found` / `tls-error` red) and a "Check URL
    now" button.
27. **`ArtifactView.tsx` branch** (5c). Dispatch on `shape`;
    swap Phase 2's placeholder strings for the new components.
    Review pane + outgoing links stay shape-agnostic.
28. **`NewArtifactDialog.tsx` three-tab extension** (5c).
    Markdown / Upload file / Link URL tabs.
29. **Bulk URL-check UI** (5c) on `CollectionPage`. Button
    visible only when the collection contains ≥ 1 URL
    artifact; live progress indicator.
30. **`gix` dependency** (5d) added with default-features-off;
    `["blob-diff", "revision"]` feature set.
31. **Git-history service** (5d) in `src/git_history/mod.rs`.
    `list_artifact_commits` and `read_blob_at_commit` over a
    repo-handle cache on `AppState`.
32. **`GET /api/artifacts/:uuid/history`** (5d). Serves the
    diff-view dropdown.
33. **`at=<oid>` extensions** (5d) on `GET /api/artifacts/:uuid`
    and `GET /api/artifacts/:uuid/blob`. Historical
    `ArtifactDetail` / binary at a specific commit.
34. **Shape-aware diff endpoint** (5d) in `src/diff/mod.rs`.
    `GET /api/artifacts/:uuid/diff?from=<oid>&to=<oid|current>`
    returns structured DTOs per shape — content uses the
    `similar` / `imara-diff` crate, blob reports
    size/hash/media-type deltas + preview URLs, URL reports the
    string diff plus the external-content disclaimer.
35. **`SinceLastApprovalDiff` switch-over** (5d). Phase 4c's
    LCS impl goes away; the component now calls
    `GET /api/artifacts/:uuid/diff` against the last-approval
    commit. Falls back to the Phase 4b approval snapshot when
    git history can't resolve the commit (shallow clone edge
    case), with the banner worded per the locked decision.
36. **`DiffView` component** (5d) in
    `frontend/src/routes/artifact-detail/DiffView.tsx`.
    Branches on the response's `shape`.
37. **Standalone `/artifacts/:uuid/diff` route** (5d). History-
    dropdown pickers for "from" and "to" populated from
    `/api/artifacts/:uuid/history`.
38. **Dockerfile additions** (5d). Runtime stage installs
    LibreOffice-headless, libvips, ImageMagick. CI smoke test:
    build image, upload docx, fetch thumbnail, assert 200 +
    `image/png`.
39. **Fixture augmentation** (5d) in
    `.reqforge-workspace/example-test-repos/sample-project/`.
    A tiny PDF blob artifact, a URL artifact, at least one
    blob-replace commit and one URL-edit commit in history so
    the diff view has real data.
40. **End-to-end smoke test** (5d) in
    `frontend/tests/e2e/artifact-shapes.test.ts`. Upload blob
    → render → replace → diff shows size change → create URL
    artifact → click "Check URL now" → status pill flips
    green. Selenium-gated like the 4d e2e.
41. **Coverage pass + Phase 5 "Shipped" note** (5d). Rerun
    `make test` + `make fmt-check` + `make lint`; confirm line
    coverage stays ≥ 80 % (target: match Phase 4's 84.43 %).
    Update this block with the final shipped note per the
    Phase 3 / Phase 4 template.
42. **Risk Check 5** (5d; see also the Early Risk-Adjusted
    Checks section). Author per-platform scripts under
    `scripts/risk-check-5/` — Linux, macOS + Docker Desktop,
    Windows + Docker Desktop — mirroring the Risk Check 2
    layout so the maintainer doesn't have to retype the
    diagnostic steps on each host. The maintainer will want
    step-by-step directions alongside the scripts on each
    platform, per the Risk Check 2 precedent.

**Status (Phase 5):** Shipped (pending the three Risk Check 5
live runs per platform). Backend 284 tests / frontend 96 tests
green after 5a / 5b / 5c / 5d.1 / 5d.2 / 5d.3; backend line
coverage 80.67 % and function coverage 77.73 %. The dip from
Phase 4's 84.43 % is dominated by the thumbnail providers
(libvips + LibreOffice shell-outs that can only run inside the
shipped container) and gix-backed history / diff paths that
require a real mounted repo; the in-code paths exercised by the
unit + integration suites still cover every handler. 5a
introduced the flat-sibling sidecar layout, the blob + URL
loaders with hash + size stat, the `.reqforge.json` walker
dispatch, the extension allowlist (`pdf/docx/xlsx/pptx/png/
jpg/jpeg/gif/svg`), `LoadedProject::git_repo_path()`, and the
shape-aware write path (`write_sidecar_only`,
`write_blob_and_sidecar`). 5b wired the REST surface: multipart
blob upload + replace + download + stream, URL create +
/check-url + bulk /check-urls, `UpdateArtifactRequest.url`
with stale-check clearing, the `DefaultBodyLimit` sized from
`REQFORGE_MAX_BLOB_BYTES` (default 50 MiB), and the ten-variant
`CheckOutcome` classifier over the HEAD-then-GET URL checker.
5c added the thumbnail pipeline (`ThumbnailProvider` trait with
semaphore-of-2 concurrency + DashMap coalescing + sharded LRU
cache at `<workspace>/thumbnail-cache/`), probe-registered
libvips + LibreOffice providers (soft-fail when absent), the
`GET /thumbnail` endpoint with structured 404 reasons, the
`BlobArtifactView` three-tier preview (inline / thumbnail /
icon-only), `UrlArtifactView` with the colour-coded status
pill, `ReplaceBlobDialog`, the three-tab `NewArtifactDialog`
(Markdown / Upload file / Link URL), and the collection-level
`BulkUrlCheckButton`. 5d.1 added gitoxide (default-features-off,
blob-diff + revision), the `RepoCache` on `AppState` with
evict-on-publish, `list_artifact_commits` + `read_blob_at_commit`,
the `similar`-backed content diff + shape-aware blob/url diff,
the `/history` / `/diff` / `?at=<oid>` endpoints with
`fallbackReason` semantics. 5d.2 shipped the frontend
`DiffView` (content/blob/url branches + fallback banner) used
from both the standalone `/artifacts/:uuid/diff` route (with
history-dropdown pickers) and the review pane's "Since last
approval" block (the client-side LCS transform survives as an
approval-snapshot helper — no git round-trip required for that
surface). 5d.3 added the runtime Dockerfile dependencies
(LibreOffice + libvips-tools + ImageMagick + git), the
`/var/lib/reqforge` workspace path for the thumbnail cache,
the DES (blob) + REF (URL) sample fixtures, the
Selenium-gated `artifact-shapes.test.ts` e2e smoke, and
per-platform Risk Check 5 scripts under
`scripts/risk-check-5/` (`risk-check-5.sh` for Linux + macOS +
WSL2 Ubuntu, `risk-check-5.ps1` for Windows + Docker Desktop).

Outstanding user-side work before calling Phase 5 done for good:
run `scripts/risk-check-5/risk-check-5.sh` (or `.ps1`) on each of
Ubuntu, macOS + Docker Desktop, and Windows + Docker Desktop,
paste the three summary lines the scripts print into a
risk-check-5 results block, and record any platform-specific
surprises alongside. The ROADMAP entries above cite Risk Check 5
without a canonical result table — that block is the canonical
artifact.

#### Risk Check 5 results

- **macOS (26.4.1) / Docker Desktop:** blob upload →
  `/blob` content-type=`application/pdf`;
  `/thumbnail` = 500 (script-fixture artefact — the synthetic
  minimal PDF the script embeds has a valid `%PDF-` header but no
  page objects, so libvips correctly fails to rasterise it; both
  providers registered cleanly at container startup);
  URL check → `checkStatus=ok`;
  `/diff` shape=blob, before=301 after=68 (byte-size delta after
  blob replace).
- **Windows 11 Pro / Docker Desktop:** blob upload →
  `/blob` content-type=`application/pdf`;
  `/thumbnail` = 500 (same script-fixture artefact as macOS — not
  a pipeline failure);
  URL check → `checkStatus=ok`;
  `/diff` shape=blob, before=301 after=68.
- **Linux (Ubuntu 25.10):** blob upload →
  `/blob` content-type=`application/pdf`;
  `/thumbnail` = 500 (same script-fixture artefact as macOS +
  Windows — not a pipeline failure);
  URL check → `checkStatus=ok`;
  `/diff` shape=blob, before=301 after=68.
  Getting here took four fixes layered on top of each other
  (commits `db1e455` / `476ba20` / `8f88a86` / `6df0c0b`): the
  `mktemp`-without-`.pdf`-suffix upload bug, the nonroot
  container user losing `readdir` on a 0700 bind mount, the
  follow-up `chmod a+rwX` for userns-remap hosts, and — the
  real backend discovery — `atomic_write` persisting files at
  mode 0600 so host UID 1000 couldn't read what the container's
  remapped subuid had written. atomic_write now fixes files to
  0644 before persist. The backend bug was always there;
  Phase 5's risk-check was the surface that finally exposed it.

### Phase 6a — Report classes

**Outcome:** Coverage matrix, impact analysis, orphans (both
link-graph and filesystem kinds per `REPORT-orphans`), conflicts,
cycles, review status, unresolved links. All viewable in-UI with the
scope selector (per `REPORT-scopeSelector`) and the inactive-filter
toggle defaulting off (per `UX-showInactiveDefault`).

Split into four commit-sized sub-phases on the single branch:
**6a.1** shared infrastructure plus the two cheapest reports
(unresolved-links, link-graph-orphans); **6a.2** cycles plus
conflicts (still graph-only); **6a.3** coverage matrix plus
impact analysis (graph traversal, configurable covering-link-type
set); **6a.4** review status plus filesystem orphans plus the
"Adopt as artifact" wizard plus the Phase 6a shipped note. Each
sub-phase stands on its own — 6a.1 lands a usable reports index
with two live reports, later sub-phases grow the catalog without
touching the shared plumbing.

**Design decisions locked in before coding starts:**

- **Unified endpoint.** `GET /api/reports/{kind}?scope=...
&includeInactive=false&...` dispatches on the URL kind
  segment. One handler behind a `ReportKind` enum, one tagged
  response DTO per kind (`serde(tag = "kind")`). Adding a new
  report class later means a new enum variant plus a new DTO
  variant — no new endpoint surface.
- **Scope selector MVP.** System / Project / Collection only.
  The broader "user-defined filter by artifact type / review
  state / link presence" hinted at in `REPORT-scopeSelector`
  is deferred to Phase 6b; reports' queries still accept a
  `?filter=` param for forward-compat, but 6a's UI only wires
  the three-level picker.
- **Inactive-filter: single shared toggle.** A top-level
  `includeInactive` boolean on every report, defaulting `false`,
  applied uniformly (hides inactive artifacts from both the
  report's inputs and its outputs). Matches
  `UX-showInactiveDefault`'s "one toggle everywhere" intent;
  reports that grow finer-grained semantics later can layer
  their own toggles on top without breaking the shared one.
- **Covering link types for the coverage matrix.** Default set
  `{satisfies, verifies}` per `REPORT-coverageMatrix`; per-view
  override via a multi-select against the effective link
  catalog. Persisted through the saved-report-config facility
  introduced in 6a.1.
- **Saved report configs.** Per-kind JSON under
  `<workspace>/report-configs/<kind>.json` — one blob per report
  kind that records the last-used scope, options, and
  inactive-filter. Autopopulated on every report view so
  navigation lands on the user's previous settings; edits
  persist on next change. A single "reset to defaults" action
  clears the blob. Behind the same workspace-dir plumbing that
  powers reviewers.json and the thumbnail cache.
- **Filesystem-orphans walk.** On-demand per report view — no
  background watcher. The handler walks every blob-holding
  collection (one whose directory contains at least one
  `.reqforge.json` sidecar or any file matching the
  blob-extension allowlist) and reports the set symmetric-
  difference between binaries and sidecars. Results are not
  cached; the walk is cheap (stat-only, no hashing) and the
  report shows what's true right now.
- **"Adopt as artifact" wizard.** Lives in Phase 6a.4 next to
  the filesystem-orphans report. Backed by a new
  `POST /api/artifacts/{slug}/{prefix}/artifacts/blob/adopt`
  endpoint that takes an already-on-disk binary path + metadata
  (name, title, tags, description) and writes the sidecar only
  — no multipart upload, no data copy. Reuses the Phase 5a
  sidecar writer and the extension allowlist.
- **Route layout.** Single `/reports` index page plus one route
  per report kind (`/reports/unresolved-links`,
  `/reports/coverage-matrix`, etc.). Sidebar gains a "Reports"
  entry alongside Projects / Review Queue.
- **Cycle detection strategy.** Per link type that's declared
  acyclic in the effective link catalog, run an iterative DFS
  over the directed edge-set restricted to that type. Report
  each cycle as an ordered list of artifact UUIDs. Cap at 100
  cycles per link type to protect against pathological inputs.
- **Impact-analysis traversal direction.** Default = "dependents"
  (who transitively links _to_ the seed). A per-view switch can
  flip to "dependencies" (who the seed transitively links _out
  to_), matching the two questions operators typically ask
  ("what breaks if I change X" vs "what does X rely on").

**Tasks:**

1. **Unified `ReportKind` enum + `ReportRequest` DTO** (6a.1) in
   `src/reports/mod.rs`. Serde-tagged union carrying scope,
   `includeInactive`, and per-kind options. Pure shape — no
   handlers yet.
2. **`ReportResponse` DTO union** (6a.1) — one variant per report
   kind, shape-tagged identically to Phase 5d's `ShapeDiff`.
3. **Saved-report-config store** (6a.1) in
   `src/reports/saved_config.rs`. Read/write
   `<workspace>/report-configs/<kind>.json` with atomic write +
   0644 mode (per the Phase 5d `atomic_write` invariant).
4. **`GET /api/reports/{kind}` handler** (6a.1) dispatching on
   `ReportKind`; initially stubbed for every kind except the two
   that 6a.1 ships. `?scope=system|project:<slug>|collection:
<slug>/<prefix>` and `?includeInactive=true|false` query
   params.
5. **Unresolved-links report** (6a.1) — walk the UUID index,
   group unresolved `LinkView`s by source artifact, include
   the hint + the mount required to resolve.
6. **Link-graph-orphans report** (6a.1) — set of artifacts with
   zero incoming and zero outgoing links after the
   `includeInactive` filter.
7. **`ReportsIndexPage`** (6a.1) at `/reports` — tile grid of
   the seven report kinds with a one-line description each;
   tiles link to the per-kind routes.
8. **`ScopeSelector` component** (6a.1) — three-level cascader
   (System / Project / Collection) driven by the mounts DTO;
   chosen scope writes to URL search params so refresh + back
   work.
9. **`ReportHeader` component** (6a.1) — report title +
   `ScopeSelector` + `includeInactive` toggle + reset-config
   action. Wired to `useSavedReportConfig` hook that reads /
   persists the per-kind blob.
10. **`UnresolvedLinksReport` + `LinkOrphansReport` pages**
    (6a.1) on `/reports/unresolved-links` and
    `/reports/link-orphans`. Table rendering; empty-state
    copy-through from `REPORT-unresolvedLinks` /
    `REPORT-orphans`.
11. **Sidebar "Reports" entry** (6a.1) in `AppShell`.
12. **Cycles report** (6a.2) — per-acyclic-link-type DFS with
    the 100-cycle cap.
13. **Conflicts report** (6a.2) — pair-list on the
    `conflicts-with` link type.
14. **`CyclesReport` + `ConflictsReport` pages** (6a.2).
15. **Coverage-matrix report** (6a.3) — grid of parent rows
    and covering-link-type columns; cell counts covering
    children; parents with zero covering children flagged as
    gaps. Options: covering link types (multi-select), hide
    parents with no links at all (checkbox).
16. **Impact-analysis report** (6a.3) — seed-artifact picker
    (reuses `LinkPicker`'s target-search), direction switch
    (dependents / dependencies), tabular transitive output.
17. **`CoverageMatrixReport` + `ImpactAnalysisReport` pages**
    (6a.3).
18. **Review-status report** (6a.4) — aggregation grouped by
    project / collection / shape with approved / rejected /
    unreviewed / re-requested counts.
19. **Filesystem-orphans report** (6a.4) — missing-binary and
    missing-sidecar lists, surfaced per collection.
20. **`POST /api/projects/{slug}/collections/{prefix}/artifacts/blob/adopt`**
    (6a.4) — sidecar-only writer for an on-disk binary path
    that's inside the collection and passes the extension
    allowlist.
21. **`AdoptOrphanDialog`** (6a.4) on the filesystem-orphans
    report — name / title / description form wired to the
    adopt endpoint.
22. **`ReviewStatusReport` + `FilesystemOrphansReport` pages**
    (6a.4).
23. **Coverage pass + Phase 6a "Shipped" note** (6a.4). Rerun
    `make test` + `make fmt-check` + `make lint`; confirm line
    coverage stays ≥ 80 % (target: recover toward Phase 4's
    84.43 %). Update this block with the final shipped note
    per the Phase 3 / 4 / 5 template.

**Status (Phase 6a):** Shipped. Backend 346 tests (214 unit + 132
integration) / frontend 117 tests green after 6a.1 / 6a.2 / 6a.3
/ 6a.4; backend line coverage 82.99 % and function coverage
79.48 %, both up meaningfully from Phase 5's 80.67 / 77.73. All
eight report classes are live through a single
`GET /api/reports/{kind}` endpoint that dispatches on a tagged
`ReportResponse` union; per-kind saved configs round-trip
through `GET/PUT/DELETE /api/reports/{kind}/config` under
`<workspace>/report-configs/<kind>.json` (mode 0644 via the
Phase 5d `atomic_write` fix).

6a.1 introduced the shared infrastructure: `ReportKind` +
`Scope` parser (system / project / collection), `ReportQuery`
DTO, tagged `ReportResponse` union, `saved_config` store,
`ReportHeader` + `ScopeSelector` frontend shell, `/reports`
index with tile grid, sidebar "Reports" entry — plus the two
cheapest reports (unresolved-links, link-orphans) as
proof-of-shape. 6a.2 added cycles (three-colour DFS per
acyclic link type with rotation-invariant dedupe plus a
100-cycle cap plus a `truncated` flag) and conflicts
(UUID-sorted pair dedupe plus a `bidirectional` flag, 500-pair
cap). 6a.3 added coverage
matrix (default covering set `{satisfies, verifies}` per
REPORT-coverageMatrix, configurable per-view, unknown-types
echoed separately so saved-config typos surface as an amber
warning rather than dropped reports) and impact analysis (BFS
from a seed along incoming edges by default, outgoing via
`?direction=dependencies`, with depth + per-node link-type
attribution; `MAX_IMPACTED_ARTIFACTS=5000` ceiling). Bonus bug
fix during 6a.3: the cycles report now emits the
canonicalised rotation so different DFS start points produce
identical output, not just identical dedupe keys. 6a.4
rounded out the catalog with review status (approved /
rejected / re-requested / never-reviewed counts faceted by
project / collection / shape via the Phase 4a
`derive_review_state`) and filesystem orphans (on-demand fs
walk of blob-holding collections; two lists — missing-sidecar
binaries + missing-binary sidecars — per REPORT-orphans). The
`AdoptOrphanDialog` posts to a new sidecar-only
`POST /api/projects/:slug/collections/:prefix/artifacts/
blob/adopt` endpoint that validates the declared path stays
inside the target collection, passes the Phase 5a extension
allowlist, and writes the sidecar without copying the
binary; the artifact detail then renders via the same Phase
5c `BlobArtifactView` as any other uploaded blob.

No new risk-adjusted checks were needed for 6a.

### Phase 6b — Report exports

**Outcome:** HTML, CSV (per report kind's tabular representation
from `REPORT-baselineExports`), and JSON export actions on every
report. PDF (`REPORT-pdfExport`), CLI headless (`REPORT-cliExport`),
publishable HTML site (`REPORT-publishSite`), and regulatory-
formatted outputs (`REPORT-regulatoryOutputs`) are explicitly
out of scope here — each specs itself as deferred.

Split into two commit-sized sub-phases on the single branch:
**6b.1** backend export surface — all three formats for every
report kind, with Cycles declining CSV per the locked decision;
**6b.2** frontend Export menu on every report page plus the
Phase 6b shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Endpoint shape.** Path-based per-format:
  `GET /api/reports/{kind}/export.{ext}` with `ext` one of
  `json`, `csv`, `html`. Browsers clicking an `<a href>` get
  a natural `.csv` / `.html` / `.json` download filename via
  the URL; content-negotiation via `Accept` is avoided because
  it doesn't survive a direct click. Unknown `ext` → 404.
- **HTML hyperlinks.** Absolute URLs built from a new
  `REQFORGE_EXTERNAL_URL` env var. Default is empty, which
  produces same-origin relative paths (work when the HTML is
  re-served through ReqForge, broken when opened offline — an
  accepted tradeoff so operators without an external-facing
  URL still get a usable snapshot). Operators with a real
  deployment URL set `REQFORGE_EXTERNAL_URL=https://
reqforge.example.com` and the exported HTML becomes
  portable.
- **Coverage-matrix CSV layout.** Compact form: rows = parent
  artifacts, columns = covering link types, cells = count of
  covering children via that type. Sparse parent × child grid
  would blow up on real-world repos. A final summary column
  ("Has gap?") makes the gap rows grep-able from the CSV.
- **Impact-analysis CSV.** Ships as a row-per-impacted-
  artifact CSV (depth, project, collection, artifact, title,
  link types). The seed is echoed in the filename and a
  leading `# seed=…` comment line so the CSV stays complete
  without losing context. Tentative call — if operators find
  the comment-line preamble awkward we switch to a separate
  metadata JSON next to it.
- **Cycles CSV.** Declines with HTTP 406 and a structured
  JSON body pointing the caller at `json` or `html`. The
  nested-cycle shape has no flat encoding that isn't
  actively misleading, and the spec's "may decline" clause
  exists for exactly this case.
- **Filesystem-orphans CSV.** Two logical tables in one file
  via a section-header row pattern (`#### missing-sidecar`
  then the rows, blank line, `#### missing-binary` then the
  rows). Column schema is stable per section.
- **Filename on download.** Full form:
  `reqforge-<kind>-<scope-slug>-<utc-timestamp>.<ext>`, e.g.
  `reqforge-coverage-matrix-collection-sample-REQ-20260422T030000Z.csv`.
  Scope slug is the path-safe form of the scope: `system`,
  `project-<slug>`, or `collection-<slug>-<prefix>`. The
  timestamp makes consecutive downloads distinct by default
  so operators don't accidentally overwrite an earlier
  snapshot.
- **Content-Disposition + MIME.** Every export returns
  `Content-Disposition: attachment; filename="…"` so the
  browser downloads rather than rendering. MIME types:
  `application/json` for JSON, `text/csv; charset=utf-8` for
  CSV, `text/html; charset=utf-8` for HTML.
- **Matrix rendering in HTML.** The coverage-matrix HTML uses
  a real `<table>` with `<thead>` + `<tbody>`, column
  headings per covering link type, one row per parent, and
  `<a href>` links on every artifact name — the hyperlinked-
  navigation requirement from `REPORT-baselineExports` is the
  whole point of the HTML format.
- **CSV crate.** Use the `csv` crate for correctness —
  quoting, embedded newlines, UTF-8 BOM optional. Bypasses
  the hand-rolled-escaping trap.
- **Saved-config scope on export.** The export endpoints
  honour `?scope=`, `?includeInactive=`, `?coveringLinkTypes=`,
  `?seed=`, `?direction=` identically to the JSON endpoint —
  exports are always of a specific report configuration, not
  "whatever the saved-config says right now" (which can drift
  between clicks). Saved-config only populates the UI
  defaults; the actual URL fed to the export links is
  frozen at click time.

**Tasks:**

1. **`src/exports/` module** (6b.1) with `csv.rs`, `html.rs`,
   `filename.rs` submodules. Each report kind implements its
   own CSV + HTML renderer behind a single dispatch function
   keyed on `ReportResponse` variants.
2. **`REQFORGE_EXTERNAL_URL` env var** (6b.1) on
   `ServerConfig`, threaded into `AppState` and the HTML
   renderer. Empty default means same-origin relative paths.
3. **Filename builder** (6b.1) in `exports::filename`. Takes
   `(ReportKind, &Scope, Utc::now())`, returns
   `reqforge-<kind>-<scope-slug>-<YYYYMMDDTHHMMSSZ>.<ext>`.
4. **`GET /api/reports/{kind}/export.{ext}` handler** (6b.1).
   Dispatches on ext, reuses the existing `run_report` to
   compute the `ReportResponse`, serialises per the format,
   attaches `Content-Disposition` + correct MIME.
5. **JSON export** (6b.1). Trivial — reuse the existing
   serialised body with `application/json` + attachment
   disposition.
6. **CSV export — list kinds** (6b.1). Unresolved-links,
   link-orphans, conflicts, review-status, impact-analysis,
   filesystem-orphans. One row per entry per the locked
   decisions above.
7. **CSV export — coverage-matrix** (6b.1). Compact parent ×
   link-type matrix with a trailing `Has gap?` column.
8. **CSV decline for cycles** (6b.1). Return 406 with a JSON
   body `{error: "...", alternatives: ["json", "html"]}`.
9. **HTML export — shared scaffolding** (6b.1). Single-page
   template with inline CSS, title + scope + timestamp
   header, one report-kind-specific body block, and an
   `<a href>` helper that produces
   `<external_url>/projects/<slug>/collections/<prefix>/
artifacts/<name>`.
10. **HTML export — per-kind bodies** (6b.1). One renderer
    per `ReportResponse` variant. Reuses the same
    breadcrumb / pill patterns the in-UI pages use so the
    HTML and the live UI feel coherent.
11. **Integration tests** (6b.1) covering:
    - JSON export of every kind returns 200 + correct
      `Content-Type` + `Content-Disposition`.
    - CSV export of the seven supporting kinds returns 200 +
      `text/csv` with the expected column header row.
    - CSV export of cycles returns 406 with the structured
      body.
    - HTML export returns 200 + `text/html` with the report
      title in the body and at least one hyperlinked
      artifact breadcrumb.
    - Unknown `ext` returns 404.
    - Filename matches the locked pattern.
12. **`ExportMenu` component** (6b.2) on `ReportHeader`.
    Three links (JSON / CSV / HTML) with the right path per
    report kind and the current scope + option query string
    frozen into each link. CSV link is disabled on the
    Cycles report with a tooltip pointing at JSON / HTML.
13. **Route exclusion** (6b.2). `ExportMenu` reads the
    current `scope`, `includeInactive`, and any kind-specific
    options from props so the downloaded file matches what's
    on screen. No fetch dance — just an `<a href>` per link.
14. **Frontend tests** (6b.2) — ExportMenu renders three
    links with the expected URLs, cycles page shows the
    disabled CSV link + tooltip.
15. **Coverage pass + Phase 6b "Shipped" note** (6b.2).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm line coverage stays ≥ 80 %. Update this block
    with the final shipped note.

**Status (Phase 6b):** Shipped. Backend 372 tests / frontend 122
tests green after 6b.1 + 6b.2; backend line coverage 81.51 %
and function coverage 79.04 %, both over the ≥ 80 % / near-80 %
gate and close to Phase 6a's 82.99 / 79.48. All three baseline
export formats from REPORT-baselineExports are live via the
single path-based endpoint `GET /api/reports/{kind}/export/
{ext}` with ext ∈ {json, csv, html}.

6b.1 introduced the backend surface: the `csv` crate dep, a new
`src/exports/` module (`ExportFormat` enum + `CsvOutcome`
dispatch, filename builder producing the locked
`reqforge-<kind>-<scope-slug>-<UTC-stamp>.<ext>` form, per-kind
CSV renderers, single-page HTML renderer with inline CSS and
hyperlinked artifact breadcrumbs), and the
`REQFORGE_EXTERNAL_URL` env var threading through `ServerConfig`
/ `DiscoveryConfig` / `AppState` so deployed instances emit
absolute links while dev machines get same-origin relative.
Cycles declines CSV with HTTP 406 + structured
`{error, alternatives: ["json", "html"]}` per the locked
decision; coverage-matrix uses the compact parent × link-type
layout with a trailing `has_gap` column; impact-analysis carries
a leading `# seed=…` comment so the CSV is self-describing;
filesystem-orphans splits into two labelled sections in one
CSV.

6b.2 shipped the frontend: `ExportMenu` component on the shared
`ReportHeader` renders three `<a href>` download links (JSON,
CSV, HTML) with the current scope + include-inactive + per-kind
extras (coverage-matrix covering types, impact-analysis seed +
direction) frozen at render time, so clicking produces a
snapshot of what's on-screen rather than following later
saved-config drift. The Cycles report's CSV link renders as an
aria-disabled pseudo-button with a tooltip pointing at JSON /
HTML — we preempt the 406 the backend would return on a real
click.

Explicitly deferred per their own specs and untouched in 6b:
PDF export (`REPORT-pdfExport`), headless CLI mode
(`REPORT-cliExport`), publishable HTML site
(`REPORT-publishSite`), regulatory-formatted outputs
(`REPORT-regulatoryOutputs`).

No new risk-adjusted checks were needed for 6b.

### Phase 7a — Graph canvas view

**Outcome:** React Flow-based graph canvas (per
`UX-linkCreationGraph`), force-directed default, hierarchical layout
option for acyclic link types, 500-node soft cap with filter prompt.
Link authoring via drag-to-link + a minimal link-type picker dialog.

Split into three commit-sized sub-phases on the single branch:
**7a.1** backend graph endpoint + DTOs + tests; **7a.2** frontend
read-only canvas (React Flow + both layouts + filters + 500-node
banner + node-click navigation); **7a.3** drag-to-link authoring
plus the Phase 7a shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Endpoint shape.** Single `GET /api/graph` with query params
  mirroring the Phase 6a reports shape for consistency:
  `?scope=system|project:<slug>|collection:<slug>/<prefix>`,
  `?includeInactive=false`, and two new per-view filters —
  `?linkTypes=a,b,c` (CSV of link-type names) and `?tags=x,y`
  (CSV of tag strings). Defaults: system scope, inactive
  excluded, all link types, all tags.
- **Node-set definition for the 500 cap.** Every in-scope
  artifact counts, including nodes with zero edges. The cap is
  a rendering-complexity bound, not a graph-connectivity bound
  — operators who've miscategorised an orphan want to see it
  on the canvas.
- **Cap overflow behaviour.** Return the first 500 nodes by a
  stable ordering (project_slug, collection_prefix,
  artifact_name) plus `truncated: true` and `totalNodes` so
  the UI can render a "showing 500 of N; apply filters to
  narrow" banner over a usable sample. Edges are filtered to
  those whose both endpoints are in the truncated set so the
  subgraph stays internally consistent.
- **Hierarchical auto-trigger.** The response carries a
  `hintAllEdgesAcyclic: bool` computed from the effective
  link-type catalog. The frontend defaults the layout to
  hierarchical when it's true and to force-directed otherwise;
  a user toggle overrides in either direction.
- **Layout engines.** `dagre` for hierarchical (tree layout,
  small dep, well-maintained). `d3-force` for force-directed
  (the classic simulation; converges in a few hundred
  iterations on 500-node graphs). Both are small npm adds.
- **Drag-to-link UX.** React Flow's `onConnect` fires when the
  user drags an edge between two nodes. We open a minimal
  dialog with a single `<select>` of the effective link-type
  catalog plus Cancel / Create. No explanation field (keep
  the modal tight — operators who want notes can edit the
  full artifact).
- **Self-links.** Rejected at the `onConnect` handler with a
  toast ("self-links aren't supported"). Matches the existing
  backend validator's behaviour so there's no round-trip to
  learn that.
- **Authoring round-trip.** The picker dialog calls the
  existing `PUT /api/artifacts/:uuid` with the source
  artifact's links array plus the newly-added entry; no new
  endpoint. On success, the canvas refetches the graph so
  the new edge appears without a manual reload.
- **Route layout.** `/explore/graph` (path prefix leaves room
  for `/explore/matrix` in Phase 7b — avoids a rename later).
  Sidebar gains a top-level "Graph" entry below "Reports";
  the matrix view in 7b will slot in as a sibling.
- **Tag filter source.** Every node DTO carries its
  `tags: string[]`; the frontend union-derives the filter
  dropdown from the current response rather than needing a
  separate `/api/tags` endpoint. Cheap because the truncated
  response is at most 500 nodes.

**Tasks:**

1. **`src/graph/` module** (7a.1) with a single `compute` fn
   that takes the `World`, a `GraphQuery`, and returns the
   `GraphResponse` shape. Mirrors the Phase 6a reports module
   layout so the two live side-by-side cleanly.
2. **`GraphQuery` DTO** (7a.1) on `src/graph/mod.rs`. Serde-
   deserialized from the axum query extractor; parses scope
   via the reuse of `reports::Scope::parse`.
3. **`GraphResponse` + node/edge DTOs** (7a.1). Node DTO reuses
   the Phase 6a `CycleNode` shape plus `tags`. Edge DTO is
   `{ source_uuid, target_uuid, link_type, acyclic }`. Report
   also carries `total_nodes`, `truncated`, and
   `hint_all_edges_acyclic`.
4. **500-node cap + stable ordering** (7a.1) in the compute
   fn. Constant `GRAPH_NODE_CAP = 500` exported so tests and
   the UI can share the numeric.
5. **`GET /api/graph` handler + route** (7a.1). Validates the
   scope, runs `graph::compute`, returns JSON.
6. **Integration tests** (7a.1) — scope filter, link-type
   filter, tag filter, cap behaviour on a 600-node fixture
   world, hint toggle on acyclic-only edge sets.
7. **Add React Flow + dagre + d3-force deps** (7a.2) to the
   frontend package.json. Lock versions.
8. **`/explore/graph` route + `GraphPage`** (7a.2). Shell
   with scope selector (reuse `reports/ScopeSelector`), the
   new filter row, and a React Flow canvas.
9. **Layout engines** (7a.2) in
   `frontend/src/routes/explore/graph/layouts/`. `dagre.ts`
   for hierarchical; `force.ts` running d3-force simulation
   to steady state and mapping positions back to React Flow
   nodes.
10. **Layout toggle + auto-pick** (7a.2). When the server
    returns `hintAllEdgesAcyclic: true`, default the toggle
    to hierarchical; otherwise force-directed. User flips
    freely.
11. **Truncation banner** (7a.2). When `truncated: true`,
    render a prominent amber banner "Showing 500 of N nodes
    — apply filters to narrow the set" above the canvas.
12. **Node click → detail page** (7a.2). `onNodeClick` wires
    `useNavigate` to `/projects/:slug/collections/:prefix/
artifacts/:name`.
13. **Sidebar entry** (7a.2). New "Graph" link below
    "Reports" in `AppShell`'s sidebar.
14. **Frontend tests — read-only path** (7a.2). Layout
    helpers get unit tests; the page itself gets a minimal
    render test stubbing `/api/graph` + verifying the
    truncation banner + node labels appear. React Flow
    itself is mocked at the `@xyflow/react` import boundary
    to keep the test lightweight (the library uses DOM
    measurement APIs jsdom doesn't ship).
15. **Drag-to-link dialog** (7a.3). Modal triggered by React
    Flow's `onConnect`. Type-picker select populated from
    `useLinkTypes`. Reuses the existing Phase 3b
    link-picker's validation rules where they apply.
16. **Authoring round-trip** (7a.3). The dialog calls
    `useUpdateArtifact` with the source's existing links
    array + the new entry; invalidates the graph cache on
    success so the new edge appears.
17. **Self-link rejection** (7a.3). Handled in `onConnect`
    before the dialog opens; surface a toast.
18. **Coverage pass + Phase 7a "Shipped" note** (7a.3).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm line coverage stays ≥ 80 %. Update this block
    with the shipped note per the previous template.

**Status (Phase 7a):** Shipped. Backend 390 tests (239 unit + 151
integration) / frontend 132 tests green after 7a.1 + 7a.2 + 7a.3;
backend line coverage 83.28 % and function coverage 79.77 %,
both comfortably over the ≥ 80 % / near-80 % gate and up from
Phase 6b's 81.51 / 79.04. The React-Flow graph canvas is live at
`/explore/graph` with a single `GET /api/graph` feed backing it
and drag-to-link authoring reusing the existing
`PUT /api/artifacts/:uuid` write path.

7a.1 introduced the backend graph module: a pure
`build_graph(scope, query, world)` fold that filters by scope +
includeInactive + linkTypes + tags, stable-sorts by
(project, collection, name), and truncates to
`GRAPH_NODE_CAP = 500` with a `truncated` flag and edge-endpoint
pruning so the returned subgraph stays internally consistent.
The response also carries `hintAllEdgesAcyclic` computed from
the effective link catalog so the frontend can default the
layout on DAGs, and a `referencedLinkTypes` catalog
piggy-backed for tooltips. Unknown project / collection scopes
map through typed errors to 404s mirroring Phase 6a reports.

7a.2 shipped the read-only canvas: React Flow as the canvas
primitive, `dagre` for the hierarchical TB layout (acyclic edges
only — using back-edges as layout signal confuses the user's
mental model), `d3-force` for the force-directed simulation
(300 ticks to steady state at the 500-node bound). Layout
defaults follow the server hint with a manual override; the
override resets when scope changes so the new scope's hint is
respected again. Filter row carries scope + include-inactive +
link-type chips (from the full `useLinkTypes` catalog) + tag
chips (union-derived from the current node set at the 500-node
cap). Node click routes to the artifact detail page; a
prominent amber banner surfaces when `truncated: true`.

7a.3 shipped drag-to-link authoring: React Flow's `onConnect`
opens a `LinkCreateDialog` with a link-type picker populated
from `useLinkTypes`; Create posts the source artifact's full
existing links array plus the new entry through
`useUpdateArtifact`, which invalidates every cache on success so
the new edge appears on the canvas without a manual reload.
Self-link drags are intercepted at the canvas boundary and
surface an inline auto-dismissing toast ("Self-links aren't
supported") rather than round-tripping to learn the same from
the backend validator. The dialog also blocks same-type
duplicate links on the source side and includes a tooltip hint
on directed vs. acyclic link types in the picker.

No new risk-adjusted checks were needed for 7a.

### Phase 7b — Matrix link view

**Outcome:** TanStack Virtual-based matrix (rows × columns of
artifacts) per `UX-linkCreationMatrix`. Cells surface existing
links of a chosen link type and let the operator toggle them on
or off. Axis filters narrow each side independently (scope +
tags + review state). 500-per-axis soft cap with a blocking
"apply filters" banner beyond that.

Split into three commit-sized sub-phases on the single branch:
**7b.1** backend `GET /api/matrix` endpoint + DTOs + tests;
**7b.2** frontend read-only matrix (axis filter rows +
virtualized grid + truncation banner + link-type picker);
**7b.3** interactive cell toggle round-trip plus the Phase 7b
shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Endpoint shape.** New `GET /api/matrix` rather than
  reusing `/api/graph` — the matrix has two independent scopes
  (one per axis) and two independent 500-caps (one per axis),
  whereas the graph has a single scope and a single global
  500-cap. Query params: `rowScope`, `columnScope`, `linkType`
  (single, required — a matrix is per link type), plus optional
  `rowTags`, `columnTags`, `rowReviewStates`, `columnReviewStates`,
  and `includeInactive`. Response carries `rows`, `columns`,
  `edges` (already filtered to the chosen link type and in
  row→column direction), `rowsTruncated` + `totalRows`,
  `columnsTruncated` + `totalColumns`, `referencedLinkType`,
  and `rowScope` / `columnScope` echoed back.
- **Per-axis 500 cap.** If either axis is over the cap after
  filters, the response carries `{rows: [], columns: [], edges:
[]}` with the `truncated` flags + totals set, and the
  frontend renders a blocking amber banner
  ("row axis has N items — apply filters to narrow below 500")
  without drawing a partial matrix. Drawing a partial matrix
  would mis-represent coverage (cells for hidden artifacts
  appear as "no link" even when they might carry one).
- **Link-type picker.** Single-select dropdown from the
  effective catalog. Default `satisfies` (matches the Phase 6a
  coverage-matrix report's default covering type). The picker
  is required — a matrix without a link type is meaningless.
- **Cell interaction.** Click an empty cell → confirmation
  modal ("Create `{linkType}` from `{row}` to `{column}`?")
  with Create / Cancel. Click a filled cell → confirmation
  modal ("Remove `{linkType}` from `{row}` to `{column}`?")
  with Remove / Cancel. Confirmation on both sides because
  matrix cells are small dense targets and a stray click could
  silently clobber coverage state.
- **Authoring round-trip.** Both create and remove go through
  `PUT /api/artifacts/:uuid` via the existing
  `useUpdateArtifact` mutation — the row artifact's full links
  array is rewritten with the target added or filtered out.
  Same code path the Phase 7a drag-to-link uses.
- **Self-link cells.** When row and column resolve to the same
  artifact UUID (overlapping axes), the cell renders as
  disabled grey with no tooltip-triggered modal — mirrors the
  Phase 7a self-link toast rationale so the backend validator
  never has to reject a request the UI could have prevented.
- **Axis filter UX.** Two mirror filter rows (one labelled
  "Rows", one "Columns"), each carrying: `ScopeSelector` (reuse
  from Phase 6a), include-inactive toggle, tag chips (union-
  derived from the current axis's node set like 7a), and
  review-state chips (approved / rejected / re-requested /
  never-reviewed — same states as the Phase 6a review-status
  report). Review state is a first-class filter per the UX
  spec's explicit call-out.
- **Route layout.** `/explore/matrix` (sibling to Phase 7a's
  `/explore/graph`). Sidebar gains a "Matrix" entry below
  "Graph".
- **Virtualization.** `@tanstack/react-virtual` (already on the
  dep tree from Phase 3d's artifact list). Row and column
  virtualization both needed — a 500×500 matrix is 250k cells,
  too many to mount without virtualization.
- **Row / column uniqueness.** The backend de-duplicates by
  UUID within each axis (an artifact that matches both axis
  filters appears exactly once per axis). Edges still resolve
  correctly because links are UUID-keyed.

**Tasks:**

1. **`src/matrix/` module** (7b.1) with a `build_matrix` fn
   mirroring the Phase 7a `graph::compute::build_graph` layout.
   Pure fold over the `World` snapshot + a `MatrixQuery` input.
2. **`MatrixQuery` + `MatrixResponse` DTOs** (7b.1) on
   `src/matrix/mod.rs`. Serde-deserialized from the axum query
   extractor; both scopes parsed via `reports::Scope::parse`.
   Constant `MATRIX_AXIS_CAP = 500` exported.
3. **Review-state filter** (7b.1). Reuses the Phase 4a
   `derive_review_state` helper — the per-axis filter checks
   the artifact's effective state against the requested set.
4. **`GET /api/matrix` handler + route** (7b.1). Validates both
   scopes, requires `linkType`, maps unknown link-type →
   `400 invalid link type`, unknown scope → `404` same as
   Phase 7a.
5. **Integration tests** (7b.1) — default row/column scopes,
   per-axis tag filter, per-axis review-state filter, 500-cap
   on each axis independently, link-type validation, 404 on
   unknown project / collection scope.
6. **Frontend types + `useMatrix` hook** (7b.2). Matrix DTOs
   in `api/types.ts`; `api.matrix(params)` in `client.ts`;
   cache key + hook in `queries.ts`. `MATRIX_AXIS_CAP`
   constant mirrored for banner copy.
7. **`MatrixPage` shell** (7b.2) at
   `frontend/src/routes/explore/matrix/MatrixPage.tsx`. Header,
   link-type picker, two `MatrixAxisFilters` (Rows / Columns),
   and a body switching between banner / empty-state / grid.
8. **`MatrixAxisFilters`** (7b.2). Mirror of the Phase 7a
   `GraphFilters` but scoped to a single axis and adding the
   review-state chip group.
9. **`MatrixGrid`** (7b.2). `@tanstack/react-virtual` for both
   rows and columns. Frozen header row + header column.
   Filled cells render a small colored dot; empty cells render
   blank. Hover tooltip shows the full row/column coordinates
   so operators can confirm what they're about to click on.
10. **Truncation banner** (7b.2). When either axis is over
    cap, render a single blocking amber banner — no grid at
    all, no partial draw. Copy names whichever axis (or both)
    is over the cap.
11. **Sidebar entry** (7b.2). New "Matrix" link below "Graph"
    in `AppShell`'s sidebar, between Graph and the Projects
    heading.
12. **Frontend tests — read-only path** (7b.2). The page
    stubs `/api/matrix` + `/api/link-types` and verifies:
    - Grid renders expected row/column labels at small sizes.
    - Truncation banner appears instead of grid when the
      backend returns `rowsTruncated: true`.
    - Axis filter chips narrow the backend query.
13. **Cell toggle** (7b.3). Click-to-toggle on each cell
    opens a `MatrixCellDialog` — create or remove depending
    on current state. Dialog reuses the Phase 7a
    `LinkCreateDialog` modal shell style.
14. **Authoring round-trip** (7b.3). Both create and remove
    paths call `useUpdateArtifact(rowUuid)` with the rewritten
    links array; `invalidateAll` brings the grid back in sync.
15. **Self-link cells are non-interactive** (7b.3). Cells
    where `row.uuid === column.uuid` render with a disabled
    grey background and no click handler.
16. **Frontend tests — interactive path** (7b.3). Dialog
    tests mirror the 7a.3 `LinkCreateDialog.test.tsx`
    pattern: create PUTs the expected links array; remove
    PUTs without the target.
17. **Coverage pass + Phase 7b "Shipped" note** (7b.3).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm backend line coverage stays ≥ 80 %. Update this
    block with the final shipped note per the Phase 7a
    template.

**Status (Phase 7b):** Shipped. Backend 409 tests (250 unit +
159 integration) / frontend 139 tests green after 7b.1 + 7b.2 +
7b.3; backend line coverage 83.84 % and function coverage
80.31 %, both comfortably over the ≥ 80 % / near-80 % gate and
up from Phase 7a's 83.28 / 79.77. TanStack-Virtual-backed
matrix is live at `/explore/matrix` with a dedicated
`GET /api/matrix` feed and interactive cell toggles reusing the
existing `PUT /api/artifacts/:uuid` write path.

7b.1 introduced the backend matrix module: a pure
`build_matrix(rowScope, columnScope, query, world)` fold with
per-axis scope + tag + review-state filters, independent
500-caps, and row→column edge selection narrowed to the chosen
link type. Review-state filter flows through the Phase 4a
`derive_review_state` so it reflects effective state, not raw
log content. When either axis exceeds `MATRIX_AXIS_CAP = 500`
the response carries `truncated` flags + totals + empty
rows/columns/edges so the frontend can banner without drawing
a partial (and misleading) matrix. Unknown link types and
unknown review-state filters map to typed 400s; unknown
project / collection scopes to 404s; per-axis scope-parse
failures prefix the axis tag ("row scope" / "column scope") so
operators know which filter needs adjusting.

7b.2 shipped the read-only frontend: `MatrixPage` shell with
link-type picker (defaults to `satisfies`, matching the Phase
6a coverage-matrix report's covering default) and paired
`MatrixAxisFilters` panels for Rows and Columns, each carrying
the Phase 6a `ScopeSelector` + tag chip group (union-derived
from the axis's current nodes) + review-state chip group.
`MatrixGrid` uses `@tanstack/react-virtual` on both axes with
sticky header row / column; filled cells render a sky-blue dot,
empty cells stay blank, and self-link cells (row.uuid ===
column.uuid) render disabled grey so the backend validator
never has to reject a request the UI could have prevented.
Truncation banner is blocking — no partial grid — and the
`useMatrix` hook is gated on `linkType` so the initial render
doesn't fire a 400 before the catalog loads.

7b.3 shipped click-to-toggle authoring: `MatrixCellDialog`
confirms each add or remove (matrix cells are dense click
targets and a stray click could silently clobber coverage
state) and rewrites the row artifact's full links array through
the existing `useUpdateArtifact` mutation — same round-trip the
Phase 7a drag-to-link uses. Success routes through a toast
reusing the Phase 7a `GraphToast` component; `invalidateAll`
brings the grid back in sync with the new cell state without a
manual reload.

No new risk-adjusted checks were needed for 7b.

### Phase 7c — Full-text search with Tantivy

**Outcome:** Full-text search over artifact title + short name +
body + description + tags (per `UX-search`), combinable with
structured filters (shape, review state, link presence,
active/inactive, project, collection). Tantivy in-memory index
rebuilt on startup alongside the UUID index. Tantivy's native
query syntax covers phrase queries, field-scoped queries, and
boolean operators.

Split into three commit-sized sub-phases on the single branch:
**7c.1** backend Tantivy index + `GET /api/search` endpoint +
tests; **7c.2** frontend search page at `/search` (query box +
filter row + result list + sidebar entry); **7c.3** Phase 7c
shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Endpoint separation.** New `GET /api/search` separate from
  the Phase 3 `/api/artifacts/search`. The Phase 3 endpoint is
  narrow autocomplete for the link picker — prefix-ranked
  substring match over name + title, with an `exclude` UUID
  filter. Replacing it with Tantivy's relevance-scored output
  would hurt the link-picker UX (near-matches would outrank
  perfect prefix hits). Two endpoints, two use cases.
- **Indexed fields.** `title`, `artifact_name`, `body`,
  `description`, `tags` as Tantivy TEXT (stored). The default
  query field set — what an unqualified `q` searches — is all
  five. Field-scoped queries (`title:gitNative`,
  `body:"derived from"`) still work via Tantivy's native
  parser.
- **Filter fields.** `shape` (content / blob / url),
  `review_state` (approved / rejected / re-requested /
  never-reviewed — the same four states as 7b), `project_slug`,
  `collection_prefix` as Tantivy STRING + FAST (untokenized,
  fast-access). `active` and `has_links` as I64 FAST (0/1).
  Filters combine with the `q` query via a `BooleanQuery`
  AND so a query + filter narrows both.
- **`has_links` filter.** Binary: `true` means the artifact
  has ≥ 1 outgoing link, `false` means zero outgoing links.
  Incoming-link queries are out of scope here — they'd need
  the Phase 5a incoming-links index which lives on the
  artifact detail page.
- **Review-state values.** The exact kebab-case set from 7b
  (`approved`, `rejected`, `re-requested`, `never-reviewed`).
  Filter parse rejects unknown tags with a typed 400 listing
  the invalid entries — same "surface every typo at once"
  policy as the 7b matrix endpoint.
- **Index lifecycle.** Built inside `AppState::publish` — the
  same convergence point that owns the UUID index rebuild.
  Each publish produces a fresh index in a `RamDirectory`
  (Tantivy's in-memory backend); the old index is dropped.
  On-disk persistence is explicitly deferred — the spec calls
  it out as a "later option if memory pressure warrants it".
  Rebuild-on-publish keeps the index perfectly consistent
  with the World snapshot at the cost of a full re-index per
  write; acceptable at current repo scale.
- **Response shape.** `{ totalHits, truncated, hits: [{ uuid,
projectSlug, collectionPrefix, artifactName, title, shape,
reviewState, active, score, snippet?: string }] }`. Snippet
  is an HTML-free excerpt from the artifact's `body` with the
  matching terms highlighted via `<mark>` — rendered safely
  on the frontend by whitelisting the tag, not by dangerously-
  setting-HTML the whole response.
- **Pagination.** `limit` default 50, max 200; `offset` for
  simple page navigation. `truncated: true` on any response
  where `totalHits > offset + hits.len()`. Snippet generation
  is the per-hit cost ceiling — at 200 hits + a few hundred
  bytes of body each, one request stays under the 50 ms
  budget on mid-range hardware.
- **Query parser errors.** Malformed Tantivy queries (bad
  regex, unmatched quotes) surface as a typed 400 with the
  Tantivy parser's own error message in the body. Empty `q`
  runs a match-all query so pure-filter searches still work
  ("show me every never-reviewed content artifact in REQ").
- **Route layout.** `/search` top-level (not under
  `/explore/*` because it's a text-in / list-out navigation,
  not a canvas or matrix). Sidebar gains a "Search" entry
  near the top, above Review queue.
- **Tantivy tokenizer.** Default (lowercase + whitespace
  split, no stemming). Stemming would help recall but also
  surface surprising matches for technical terms (`derives`
  → `deriv`); the dev-tool corpus is small enough that recall
  isn't the bottleneck.

**Tasks:**

1. **Add tantivy dep** (7c.1). Current version pinned in
   `backend/Cargo.toml`; add an exact minor-version constraint
   to keep future `cargo update` from silently shifting query
   parser semantics.
2. **`src/search/` module** (7c.1). `SearchIndex` struct owns
   the Tantivy `Index` + a pre-parsed `QueryParser`. `build`
   constructor takes a `World` snapshot and returns a fresh
   index. `run(query, filters) -> Result<SearchResponse,
SearchError>` is the handler-facing API.
3. **`SearchQuery` + `SearchResponse` DTOs** (7c.1). Serde-
   deserialized from the axum query extractor. Filter params
   mirror the Phase 7b matrix shape where they overlap (review
   state + scope + active).
4. **Index lifecycle** (7c.1). `AppState::publish` builds a
   fresh `SearchIndex` alongside the UUID index rebuild and
   stores it on the `World` snapshot so the snapshot remains
   self-contained (the same pattern as `World.index`).
5. **`GET /api/search` handler + route** (7c.1). Validates
   scope, parses filter CSVs with the same unknown-tag
   tolerance the 7b matrix handler uses, maps Tantivy parser
   errors to 400s, empty-scope writes to 404s.
6. **Snippet generation** (7c.1). Tantivy's `SnippetGenerator`
   against the `body` field; HTML-escape the whole excerpt
   first, then substitute `<mark>...</mark>` markers. The
   frontend whitelists the tag rather than dangerously setting
   HTML.
7. **Integration tests** (7c.1) — default-field match across
   title + name + body + tags, field-scoped query, phrase
   query, boolean AND/OR/NOT, shape filter, review-state
   filter, has-links filter, scope filter, malformed query
   → 400, unknown review-state → 400, pagination.
8. **`SearchPage` route + shell** (7c.2) at
   `frontend/src/routes/search/SearchPage.tsx`. Query box
   (300ms debounce), filter row, result list.
9. **Query box + debounce** (7c.2). `useDebounce` (existing
   hook if there's one; otherwise a small local copy). Empty
   query runs match-all so filter-only searches work.
10. **Filter row** (7c.2). Scope (reuse Phase 6a
    `ScopeSelector`), shape multi-pick, review-state
    multi-pick (reuse the Phase 7b tag list), has-links
    three-way toggle (any / has links / no links), include-
    inactive checkbox.
11. **Result list** (7c.2). Virtualized with
    `@tanstack/react-virtual` (already on the dep tree) for
    the 200-hit ceiling. Each row: project/collection/name
    mono header, title, review-state badge, shape badge, an
    optional `<mark>`-highlighted body snippet.
12. **Snippet rendering** (7c.2). Parse the server's HTML-
    escaped-plus-`<mark>` excerpt by splitting on `<mark>`
    literals and rendering spans per segment — never
    `dangerouslySetInnerHTML`.
13. **Pagination** (7c.2). Next / Previous buttons using
    `offset`; disable when the response's `truncated` flag
    is false.
14. **Sidebar entry** (7c.2). New "Search" NavLink at the top
    of the sidebar, above "Review queue".
15. **Frontend tests — result list + filters + pagination**
    (7c.2). Mock `/api/search`; assert the debounce fires
    once, filters thread into the URL, snippets render with
    `<mark>` segments as regular DOM (no innerHTML hack), and
    Next/Prev drives the `offset` param.
16. **Coverage pass + Phase 7c "Shipped" note** (7c.3).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm backend line coverage stays ≥ 80 %. Update this
    block with the final shipped note per the Phase 7a / 7b
    template.

**Status (Phase 7c):** Shipped. Backend 432 tests (262 unit +
170 integration) / frontend 148 tests green after 7c.1 + 7c.2;
backend line coverage 84.40 % and function coverage 81.02 %,
both comfortably over the ≥ 80 % / near-80 % gate and up from
Phase 7b's 83.84 / 80.31. Tantivy full-text search is live at
`/search` with a dedicated `GET /api/search` feed backing it
and the Phase 3 link-picker autocomplete left untouched for
its narrow prefix-ranking use case.

7c.1 introduced the backend index: a Tantivy `RamDirectory`
index built inside `run_discovery` alongside the UUID index
and carried on the `World` snapshot so the two views converge
on every `publish`. Indexed fields per UX-search (title,
artifact_name, body, description, tags) are the default search
targets so unqualified `q` matches anywhere; field-scoped
queries (`title:reactor`, `body:"pressure envelope"`) still
work via Tantivy's native parser. Structured filters (shape,
review_state, project_slug, collection_prefix as STRING + FAST;
active + has_links as INDEXED + FAST i64) AND onto the text
query via a BooleanQuery. Snippet generation runs against
`body` and post-processes Tantivy's default `<b>` markers into
`<mark>` so the frontend can split on the literal tag safely.
Empty `q` runs a match-all so pure-filter searches work;
malformed queries surface the parser's own message as a typed
400; unknown shape / review-state filters collect every typo
into one 400 per the Phase 7b "surface every unknown at once"
policy.

7c.2 shipped the frontend: `/search` top-level route (sidebar
entry above Review queue), a debounced query box (300 ms),
the filter row (scope, shape chips, review-state chips, three-
way has-links toggle, include-inactive), and a result list
with Prev/Next pagination. Each row carries a monospaced
project/collection/name breadcrumb linking to the detail page,
the title, a review-state badge using the shared colour
palette, a shape badge, and an optional snippet. `SearchSnippet`
splits the backend's `<mark>`-marked excerpt into span / mark
segments — no `dangerouslySetInnerHTML` anywhere — so a
hostile payload that slipped past the backend's escaping would
render as inert text rather than execute.

Explicitly deferred per the spec's on-disk-later-if-needed
note: an on-disk Tantivy index. The current RamDirectory design
rebuilds per publish and matches the UUID-index posture.

No new risk-adjusted checks were needed for 7c.

### Phase 7d — Browsable title-indexed views

**Outcome:** Per-artifact-type indexes (per `UX-browseByType`) —
one pane per distinct Collection prefix across every mounted
project, artifacts sorted by title, filterable by scope, tags,
and review state. Complements the Phase 7c search with a
scannable browse experience.

Split into two commit-sized sub-phases on the single branch:
**7d.1** backend `GET /api/browse` endpoint + DTOs + tests;
**7d.2** frontend browse page at `/browse` + sidebar entry +
Phase 7d shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Type key = Collection `prefix`.** Per ART-collectionGrouping
  a Collection is "a named, typed grouping of related
  artifacts"; the prefix appears in every artifact ID so
  operators already think in those terms. Two mounted projects
  whose Collections share a prefix land in one pane even when
  the Collections' display names differ. Display label is the
  `name` of the first-encountered Collection for the prefix;
  any inconsistent names across projects surface in an optional
  `nameVariants: string[]` field so the UI can warn without
  blocking the merge.
- **One endpoint, everything inline.** `GET /api/browse`
  returns all panes with artifacts included. The corpus is
  bounded by the in-scope artifact count (< 10k at current
  scale) which fits a single JSON payload comfortably. No
  per-pane cap in this phase — if a single pane ever pushes
  the request past the latency budget, we'll add a bounded
  mode later.
- **Query parameters.** `scope=system|project:<slug>|
collection:<slug>/<prefix>`, `tags` (CSV of tag strings;
  any-match), `reviewState` (CSV of approved / rejected /
  re-requested / never-reviewed — same kebab-case set as
  Phase 7b matrix + 7c search), `includeInactive` (default
  false). Identical semantics to the 7c search filters so
  operators don't relearn the knobs.
- **Sort order.** Within a pane: case-insensitive by `title`,
  then by `projectSlug` / `artifactName` tiebreak for stable
  output across runs. Across panes: prefix ascending.
- **Pane shape.** `{ prefix, name, nameVariants?,
totalArtifacts, artifacts: [{ uuid, projectSlug,
collectionPrefix, artifactName, title, shape, active,
reviewState, tags }] }`. `nameVariants` is only present
  when the same prefix appears under ≥ 2 distinct Collection
  names; the array lists the alternates (not the chosen
  display label).
- **Frontend route.** `/browse` top-level (sibling to
  `/search`). Sidebar gains "Browse" between "Search" and
  "Review queue".
- **Per-pane layout.** One collapsible card per pane with a
  header showing `prefix — name — N artifacts`; a small
  title-substring input lets the operator narrow within a
  pane client-side (instant feedback, cheap at typical pane
  sizes). Clicking a row routes to the artifact detail page.
- **Review-state palette.** Reuse the Phase 7c search page's
  badge colours so operators see one consistent colour
  vocabulary across browse + search.

**Tasks:**

1. **`src/browse/` module** (7d.1). `build_browse(scope,
query, world)` pure fold returning the grouped-by-prefix
   response. Review-state filter flows through the Phase 4a
   `derive_review_state`; unknown review-state tags surface
   as a typed 400 matching the 7b / 7c handlers' "report all
   typos at once" policy.
2. **`BrowseQuery` + `BrowseResponse` DTOs** (7d.1) on
   `src/browse/mod.rs`. Camel-case on the wire; scope parsed
   via `reports::Scope::parse`.
3. **Name-variant detection** (7d.1). Walk once to collect
   `{ prefix -> set<name> }`; when the set has ≥ 2 entries,
   populate `nameVariants` in the pane with every name
   except the chosen display label (the lexicographically
   first).
4. **`GET /api/browse` handler + route** (7d.1). Scope 404s,
   review-state 400s, typed errors mapped the same way the
   7c search handler does it.
5. **Integration tests** (7d.1) — default system scope,
   scope narrows to a single project, tag filter, review-
   state filter, include-inactive, name-variant surfacing,
   unknown review-state → 400, unknown project scope → 404.
6. **Frontend types + `useBrowse` hook** (7d.2) in
   `api/types.ts` / `client.ts` / `queries.ts`.
7. **`BrowsePage`** (7d.2) at
   `frontend/src/routes/browse/BrowsePage.tsx`. Header +
   filter row + one `BrowsePane` card per response entry.
8. **`BrowsePane`** (7d.2) — collapsible card with the prefix
   / name / count header, a client-side title-substring
   filter input, and the artifact row list. Name-variant
   warnings render as an amber pill next to the header.
9. **Sidebar entry** (7d.2) — "Browse" below "Search" and
   above "Review queue".
10. **Frontend tests — render + filter + name-variant
    warning** (7d.2). Mock `/api/browse` and assert: panes
    render with expected counts, scope / tag / review-state
    toggles thread into the URL, a pane with name variants
    renders the warning, the in-pane title filter narrows
    client-side without an additional network request.
11. **Coverage pass + Phase 7d "Shipped" note** (7d.2).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm backend line coverage stays ≥ 80 %. Update this
    block with the shipped note per the Phase 7a / 7b / 7c
    template.

**Status (Phase 7d):** Shipped. Backend 449 tests (271 unit +
178 integration) / frontend 153 tests green after 7d.1 + 7d.2;
backend line coverage 84.82 % and function coverage 81.46 %,
both up from Phase 7c's 84.40 / 81.02. The browse-by-type view
is live at `/browse` with a dedicated `GET /api/browse` feed
backing it, grouping artifacts by Collection prefix across
every mounted project.

7d.1 introduced the backend endpoint: a pure
`build_browse(scope, query, world)` fold that groups artifacts
by Collection `prefix` via a BTreeMap (prefix-ascending
iteration for free), applies the scope + tag + review-state +
include-inactive filter vocabulary shared with the Phase 7c
search, and emits stable-ordered panes. Within a pane,
artifacts sort case-insensitively by title with a (project,
name) tiebreak so the output stays deterministic across runs.
Name-variant surfacing: when the same prefix appears under ≥ 2
distinct Collection names across mounted projects, the pane's
`nameVariants` field lists the alternates beyond the chosen
(lexicographically-first) display label — the merge never
blocks, the drift is just flagged. Error surface matches 7b /
7c: typed 404 for unknown project / collection scope, typed
400 for unknown review-state filter tags with every typo
collected into one message.

7d.2 shipped the frontend: `/browse` top-level route (sidebar
entry between Search and Review queue), a filter row carrying
scope + include-inactive + review-state chips + tag chips
(union-derived from the current response), and one
collapsible `BrowsePane` card per response entry. Panes start
collapsed so the page scans; expanding one shows the existing
artifacts (no extra network request — the response carries
everything) and exposes an in-pane title-substring filter for
client-side narrowing. Name drift surfaces as an amber pill on
the pane header with a hover-title listing the variants.
Clicking a row routes to the artifact detail page.

No new risk-adjusted checks were needed for 7d.

### Phase 8 — Doorstop import

**Outcome:** All `INTEROP-doorstop*` requirements implemented.
After this phase, `requirements-doorstop/` can be auto-migrated
into ReqForge's own artifacts directory and retired. One-way:
round-tripping between ReqForge and doorstop is explicitly out
of scope.

Split into three commit-sized sub-phases on the single branch:
**8.1** backend parser + plan + `POST /doorstop/preview`
(read-only; no writes);
**8.2** backend execute + `POST /doorstop/import` + report +
Phase 6b JSON/CSV/HTML export integration;
**8.3** frontend import wizard launched from the project detail
page + Phase 8 shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Add `serde_yaml` dep.** Pinned to an exact minor so future
  `cargo update` doesn't silently shift YAML parser semantics
  between builds. Used by the doorstop parser only — ReqForge
  itself stays JSON-in-YAML-delimiters per FORMAT-frontmatter.
- **Two-phase import.** Preview runs parse + plan + report
  with no writes; Import runs the same pipeline and then
  writes files via the Phase 5a `write::artifact_file` +
  `write::sidecar` atomic paths and the Phase 5b URL-artifact
  writer. The preview lets operators resolve collisions and
  review the plan before any disk state changes, per
  INTEROP-doorstopPrefixCollision ("halt before writing any
  files").
- **Path safety.** The `source` parameter is interpreted
  relative to the Project's root on disk. The path must stay
  inside the project root (no `..` traversal, no absolute
  paths) — reuse the canonicalisation + stays-inside-root
  check from the Phase 6a orphan-adopt endpoint. The source
  directory itself may equal the project root.
- **Prefix collision handling.** Preview surfaces each
  collision as a structured error; import refuses to run at
  all if any collision is present (all-or-nothing per the
  spec). Operator resolves by renaming one of the prefixes or
  removing the conflicting ReqForge Collection, then re-runs.
- **ID normalisation.** Preserve numeric padding so doorstop
  `REQ001` lands as ReqForge `REQ-001` (not `REQ-1`). Replace
  `-` in the NANU portion with `_` so `DES-rocket-nozzle`
  becomes `DES-rocket_nozzle`. Stash the original UID in
  `legacy.doorstopUid` per INTEROP-doorstopIdNormalization.
- **URL-ref detection.** Explicit prefix whitelist, case-
  insensitive: `https://`, `http://`, `ftp://`, `doi:`,
  `urn:isbn:`. A scheme-plus-colon catch-all would be too
  loose (bibliographic citations often look like
  `author:1994:title`). URL-shaped refs produce a URL
  artifact plus a `cites` link; non-URL refs go into
  `legacy.ref` verbatim.
- **Synthetic review entry.** When `reviewed` is non-null,
  emit one log entry at import time: `{outcome: "approved",
reviewer: "imported-from-doorstop", timestamp: <ISO-8601
import-run time>, explanation: "Imported from doorstop;
original reviewed hash: <hash>"}`. Null `reviewed` means
  empty log — the artifact appears as unreviewed in ReqForge.
- **Report kind.** A new `ReportKind::DoorstopImport` lives
  alongside the Phase 6a catalog so the Phase 6b JSON/CSV/HTML
  export surface picks up the doorstop report at zero extra
  cost. The report is held in memory on `AppState` keyed by
  project slug; it survives the current process only. A
  persisted-to-disk report is explicitly out of scope for
  Phase 8.
- **No modification of source files.** Per
  INTEROP-doorstopPreserveOriginal the importer never touches
  the doorstop YAML files. Operators remove or archive them
  manually after verifying the imported content.
- **Frontend route layout.** Wizard is launched from a new
  "Import from doorstop" action on the project detail page
  (not a global route) since import is per-project.
- **Test fixtures.** The repo already carries a live doorstop
  tree at `requirements-doorstop/` (the project's own specs).
  The import round-trip tests use a minimal synthetic fixture
  in a temp dir — not the live tree — so fixture edits don't
  couple to spec edits.

**Tasks:**

1. **Add `serde_yaml` workspace dep** (8.1).
2. **`src/doorstop/` module — parse** (8.1). YAML readers for
   the `.doorstop.yml` marker (`settings.{prefix, parent, sep,
digits, itemformat}`) and per-item files (the doorstop
   schema: `header`, `text`, `active`, `derived`, `level`,
   `normative`, `links`, `ref`, `reviewed`, plus extension
   fields). Both tolerate the doorstop conventions
   (sometimes-scalar-sometimes-list `links`, unquoted scalars,
   etc.).
3. **`src/doorstop/ids.rs` — ID normalisation** (8.1). Pure
   functions: parse a doorstop UID into prefix + NANU,
   normalise the NANU per the spec, and rebuild the ReqForge
   name.
4. **`src/doorstop/refs.rs` — URL shape detection** (8.1).
   Case-insensitive prefix whitelist; returns an enum
   `{UrlRef(String), NonUrlRef(String)}` distinguishing the
   two outcomes.
5. **`src/doorstop/plan.rs` — plan builder** (8.1). Pure
   function over the parsed tree: discovers `.doorstop.yml`
   markers under the source root, builds the full plan
   including Collection → Collection mapping, per-item
   artifact metadata (not yet written), link translations
   (resolved against the plan's own UUID index — new UUIDs are
   assigned in a first pass so the second pass can resolve
   `links`), ref dispositions, prefix collisions, and
   unresolved-link list.
6. **`POST /api/projects/:slug/doorstop/preview`** (8.1).
   Path safety check, returns the plan + its derived report
   (collections, artifact counts, link counts, ref
   dispositions, custom-fields-preserved count, synthetic-
   review-entry count, unresolved links, warnings/errors,
   prefix collisions). No filesystem writes.
7. **Integration tests — preview** (8.1). Synthetic two-
   document fixture covering: clean discovery, prefix
   collision halts, padded numeric NANUs, dashed NANU
   underscore conversion, URL + non-URL ref split, reviewed-
   hash entry, unresolved link flagged, custom field
   preserved.
8. **`src/doorstop/execute.rs` — execute** (8.2). Takes a
   plan and writes the Collection directories, `.collection.
json` sidecars with `importNotes`, per-artifact content
   files (frontmatter + body) via `write::artifact_file`, URL
   artifacts for URL-shaped refs, and synthetic review-log
   entries. Every write uses the existing atomic paths.
   Ownership overrides apply as on any other write.
9. **`POST /api/projects/:slug/doorstop/import`** (8.2). Same
   path-safety check as preview, runs the plan, refuses if
   any collision is present, returns the final report and
   caches it on `AppState` keyed by project slug so the
   Phase 6b export surface can re-serve it.
10. **Report kind + export wiring** (8.2). `ReportKind::
DoorstopImport` + response + JSON / CSV / HTML renderers
    in `src/exports/`. CSV renderer emits sections for
    Collections, Artifacts-by-Collection, Ref-Dispositions,
    Unresolved-Links, Warnings.
11. **Integration tests — execute** (8.2). End-to-end: run
    preview then import against the synthetic fixture, assert
    every expected file exists with the expected frontmatter,
    assert the prefix-collision case refuses the import
    cleanly, assert the report exports render in each of
    JSON, CSV, and HTML without a schema mismatch.
12. **Frontend wizard** (8.3). Button on `ProjectPage` →
    opens `DoorstopImportDialog`. Three panels in sequence:
    source-path input, preview rendering (collections,
    artifacts, collisions, unresolved links), confirm +
    import → result rendering (with Phase 6b export links).
13. **`useDoorstopPreview` / `useDoorstopImport` hooks**
    (8.3) — React Query mutations wrapping the two
    endpoints.
14. **Frontend tests** (8.3). Dialog renders the preview
    collections + counts, halts on collision with an actionable
    error, surfaces unresolved-link warnings, and the import
    button invokes the import endpoint and renders the result
    report.
15. **Coverage pass + Phase 8 "Shipped" note** (8.3).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm backend line coverage stays ≥ 80 %. Update this
    block with the shipped note per the Phase 7 template.

**Status (Phase 8):** Shipped. Backend 491 tests (299 unit +
192 integration) / frontend 157 tests green after 8.1 + 8.2 +
8.3; backend line coverage 85.63 % and function coverage
81.95 %, both up from Phase 7d's 84.82 / 81.46. One-way import
from doorstop is live: the wizard at the project detail page
runs preview → import → report, writing every doorstop item
as a ReqForge content artifact (plus URL-companion artifacts
for URL-shaped refs) under the project's Collections root.

8.1 introduced the backend parser + plan:

- `doorstop::parse` reads `.doorstop.yml` markers and
  per-item YAML files via `serde_yaml`, tolerating the
  scalar / mapping-with-hash link shapes and collecting
  every unrecognised extension field for the `legacy`
  object preservation.
- `doorstop::ids` normalises doorstop UIDs into ReqForge
  names — padding-preserving (`REQ001` → `REQ-001`) and
  dashes-in-NANU → underscores (`DES-rocket-nozzle` →
  `DES-rocket_nozzle`), with the original UID recovered via
  `legacy.doorstopUid`.
- `doorstop::refs` classifies ref strings via an explicit
  URL-prefix whitelist (`https://`, `http://`, `ftp://`,
  `ftps://`, `doi:`, `urn:isbn:`) rather than a
  scheme-plus-colon catch-all that would misclassify
  bibliographic citations.
- `doorstop::plan` runs the two-pass builder — assign
  UUIDs, resolve links against the full index, flag
  collisions without short-circuiting the plan, track
  unresolved links with their hints preserved for the
  existing `TRACE-unresolvedLinks` surface.
- `POST /api/projects/:slug/doorstop/preview` returns the
  plan without writing anything; path safety mirrors the
  Phase 6a orphan-adopt handler.

  8.2 shipped execute + report + exports:

- `doorstop::execute` walks the plan and writes every
  `.collection.json` + content artifact + URL companion via
  the existing Phase 5a atomic-write path. The source
  doorstop files are never touched.
- Per INTEROP-doorstopPrefixCollision the import endpoint
  refuses with 409 if any collision is present; the
  structured body includes the full collision list so the
  wizard can render it.
- `doorstop::report` + `exports::doorstop` produce
  JSON + CSV (multi-section) + HTML (inline CSS) downloads
  via `GET /api/projects/:slug/doorstop/report/export/{ext}`,
  piggy-backing on the Phase 6b export scaffolding. The
  report is cached in memory on `AppState` keyed by project
  slug (survives the current process only; persisted-to-
  disk reports explicitly deferred).

  8.3 shipped the frontend:

- `DoorstopImportDialog` on the project detail page (button
  "Import from doorstop" alongside "New collection"). Three
  panels walk the operator through source-path input,
  preview rendering with plan summary + collision warnings,
  and a success report with the three export-format links.
- `ApiError` grew an optional structured `body` field so the
  dialog can render the 409 prefix-collision payload without
  parsing the error string a second time.
- `useDoorstopPreview` + `useDoorstopImport` React Query
  mutations. The import mutation invalidates every cache on
  success so the newly-imported artifacts surface in every
  view (graph, matrix, search, browse, report catalog)
  immediately.

Explicitly deferred per the spec's own call-out: persisted-
to-disk import reports. In-memory-only matches the UUID-
index + search-index posture.

No new risk-adjusted checks were needed for Phase 8.

### Phase 9a — Code traceability scanner

**Outcome:** The scanner subsystem that Phase 9b's
code-traceability report will read from. Language registry
(`TRACE-codeLanguageRegistry`), tag parser
(`TRACE-codeTagFormat`), scan-path resolution +
ignore-directory machinery (`TRACE-codeScanConfig`), and a
file walker that produces per-project scan output keyed by
`(projectSlug, collectionPrefix, artifactName)`. No report
rendering, no coverage-matrix integration — 9b owns both.

Split into three commit-sized sub-phases on the single branch:
**9a.1** language registry + tag parser (pure Rust, no IO) +
unit tests; **9a.2** scan-path resolution + walker +
`run_scan` integration tests against a synthetic fixture;
**9a.3** debug HTTP endpoint exposing raw `ScanOutput` +
Phase 9a shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **Module layout.** New `src/scan/` with `languages.rs`
  (registry + System-extension loader), `tags.rs` (verb +
  alias parser, comment-only recognition, multi-ID +
  trailing-comma continuation), `config.rs` (scan-path
  defaults + ignore set), `walker.rs` (walk + extension
  filter + comment extraction + tag collection). `mod.rs`
  exposes a single `run_scan(project, world) -> ScanOutput`
  that 9b consumes.
- **Language registry shape.** Typed entries with
  `name, extensions, line_comments, block_comments,
dockerfile_name_match`. Built-ins: Rust, Python (with
  triple-quoted strings treated as comments for tag
  scanning per the spec), JavaScript, TypeScript, POSIX
  shell, Dockerfile. YAML deliberately omitted per the
  spec.
- **System-declared languages.** `SystemConfig.languages`
  already exists as an opaque `Option<serde_json::Value>`;
  9a.1 adds a typed loader that parses it into
  `Vec<Language>` and rejects entries whose name matches a
  built-in with a typed error (pointing at "file a bug or
  submit a change against ReqForge itself" per the spec).
  Add-only semantics mirror the Phase 3a link-type
  extensibility decision.
- **Tag parser.** Recognises verbs case-insensitively,
  canonicalises on output. Verb set is the six built-in
  link types plus `Implements` / `Requirements` aliased to
  `Satisfies`. IDs follow the `<prefix>-<name>` form from
  the ReqForge UID convention. Multi-ID + trailing-comma
  continuation work across subsequent comment-only lines.
- **Python triple-quoted strings.** Treated as comments per
  the spec. Share the block-comment state machine by
  letting Python's `Language` entry carry `""" """` and
  `''' '''` in its `block_comments` list.
- **Scan-path resolution.** Per-project `scan_paths`
  overrides; otherwise default `["src", "tests", "lib"]`
  filtered to directories that actually exist (silent skip
  on missing defaults — a project without `tests/` is
  common and shouldn't warn). Ignore directories:
  `.git`, `node_modules`, `target`, `dist`, `build`,
  `__pycache__`, `.venv`. Hardcoded `HashSet<&'static str>`
  — no user override in 9a; add if an operator needs it.
- **Cross-project resolution.** Tags name
  `(prefix, artifactName)` without a project slug. The
  resolver scans every mounted project for matches and
  emits one entry per resolved artifact (so a multi-
  project system with the same `(REQ, REQ-001)` in two
  mounted projects surfaces both). Unresolved tags go into
  `orphan_tags` for 9b to render.
- **`ScanOutput` shape.** `{ tags_by_artifact:
BTreeMap<ArtifactKey, Vec<ScanTag>>, orphan_tags:
Vec<ScanTag>, scanned_file_count, file_errors }`.
  `ScanTag = { file: PathBuf, line: usize, verb: String,
raw_id: String }`. `ArtifactKey` is
  `(projectSlug, collectionPrefix, artifactName)` —
  enough for 9b to look the artifact up in the UUID index
  without re-walking.
- **Debug HTTP endpoint.** `GET /api/projects/:slug/
code-scan` returns the raw `ScanOutput` for a project.
  Keeps 9a end-to-end testable and gives 9b a stable
  interface to build on. Not customer-facing until 9b
  wraps it into the report.
- **No artifacts created.** Per
  `TRACE-codeScanNotArtifacts` the scanner produces overlay
  data only. The typed-link graph stays untouched.
- **Split into three sub-phases.**

**Tasks:**

1. **`src/scan/languages.rs`** (9a.1). Built-in registry
   with Rust / Python / JavaScript / TypeScript / POSIX
   shell / Dockerfile entries. `effective_languages(system)`
   merges System-declared entries with conflict detection.
2. **`src/scan/tags.rs`** (9a.1). `parse_tags(text,
comment_ranges) -> Vec<RawTag>`. Verb alias table
   (`Implements`, `Requirements` → `Satisfies`); multi-ID +
   trailing-comma continuation; ID form
   `<prefix>-<artifactName>`. Unit tests for: single tag,
   multi-ID tag, continuation across lines, case-insensitive
   verb, alias canonicalisation, tag outside a comment
   ignored, unknown verb ignored.
3. **`src/scan/config.rs`** (9a.2). `resolve_scan_paths(
project)` returning absolute paths to walk.
   `IGNORE_DIRS` constant. Default fallback to
   `["src", "tests", "lib"]` filtered to existing dirs.
4. **`src/scan/walker.rs`** (9a.2). File walker that
   matches each file against the language registry,
   extracts comment ranges, runs the tag parser, collects
   tags. Resolves tags against the world's mounted
   projects into `ArtifactKey` or orphan.
5. **`run_scan(project, world) -> ScanOutput`** (9a.2).
   Public entry point; `tags_by_artifact` keyed stably so
   the 9b report renders deterministically.
6. **Integration tests** (9a.2) — synthetic multi-language
   fixture covering: Rust line + block comments, Python
   triple-quotes, JS `/** */`, shell `#`, Dockerfile
   `#`, a tag outside a comment (ignored), an alias verb,
   an orphan tag, a trailing-comma continuation across
   lines, an ignore-directory is skipped.
7. **`GET /api/projects/:slug/code-scan`** (9a.3). Returns
   raw `ScanOutput` JSON.
8. **Coverage pass + Phase 9a "Shipped" note** (9a.3).
   Rerun `make test` + `make fmt-check` + `make lint`;
   confirm backend line coverage stays ≥ 80 %. Update this
   block with the shipped note per the Phase 7 / 8
   template.

**Status (Phase 9a):** Shipped. Backend 532 tests (332 unit +
200 integration) / frontend 157 tests green after all three
sub-phases (9a.1 + 9a.2 + 9a.3); backend line coverage 86.29 %
and function coverage 83.03 %, both up from Phase 8's
85.63 / 81.95. The code-traceability scanner subsystem is
live: Phase 9b's report reads from
`GET /api/projects/:slug/code-scan`, which returns
`ScanOutput` keyed by `(projectSlug, collectionPrefix,
artifactName)` plus separate `orphanTags` for unresolved
references.

9a.1 introduced the pure parsing subsystem:

- `scan::languages` — six built-in entries (Rust, Python,
  JavaScript, TypeScript, POSIX shell, Dockerfile) with
  line-comment markers + block-comment pairs + Python
  triple-quoted-string handling per the spec.
  `effective_languages(system)` merges
  `SystemConfig.languages` with add-only enforcement: a
  user-declared entry colliding with a built-in is rejected
  with a typed error pointing at "file a bug or submit a
  change against ReqForge instead" — matching the Phase 3a
  link-type extensibility decision.
- `scan::tags` — verb + alias canonicalisation (`Implements`
  and `Requirements` → `Satisfies`), multi-ID support, and
  trailing-comma continuation across subsequent comment-only
  lines. Verbs matched case-insensitively and
  hyphen-tolerant so `DerivesFrom` === `derives-from`.

9a.2 shipped the filesystem layer:

- `scan::config` — `resolve_scan_paths(project)` with
  defaults (src/, tests/, lib/) silently filtered to
  existing directories; declared paths with absolute or
  parent-traversal forms rejected. Ignore set is the
  spec's minimum: .git, node_modules, target, dist, build,
  \_\_pycache\_\_, .venv.
- `scan::walker` — recursive walker with a language-aware
  comment extractor. Line comments pick the longest
  matching marker (so Rust's `///` beats `//`); block
  comments scan forward for the matching close and tolerate
  unterminated blocks by running to EOF. Each `CommentRun`
  carries a `comment_only_lines` flag set only when the
  marker is first non-whitespace on its line — the tag
  parser needs this for the trailing-comma continuation
  rule.
- `scan::run_scan(project, world) -> ScanOutput` — public
  entry point. A single `ArtifactIndex` built per call
  keeps tag resolution O(1). Cross-project resolution emits
  one entry per match, so a multi-project system with the
  same `REQ-001` in two mounted projects surfaces both
  paths. Unresolved tags go into `orphan_tags` for 9b to
  render; per-file read errors go into `file_errors` so one
  bad file doesn't fail the whole scan.

9a.3 exposed the debug HTTP surface:

- `GET /api/projects/:slug/code-scan` returns the raw
  `ScanOutput` JSON. The scanner runs on a blocking thread
  pool via `spawn_blocking` so filesystem walks don't stall
  the axum worker. Unknown project → 404; a stable wire
  shape 9b consumes without further coupling.

Per TRACE-codeScanNotArtifacts the scanner produces overlay
data only. The typed-link graph stays untouched; 9b's report
is what operators see.

No new risk-adjusted checks were needed for Phase 9a.

### Phase 9b — Code traceability report integration

**Outcome:** `REPORT-codeTraceability` as a first-class report
kind generated from Phase 9a's scanner output; coverage-matrix
report learns about code-side evidence alongside artifact-side
links per the configurable covering-link-type set.

Split into two commit-sized sub-phases on the single branch:
**9b.1** backend compute + coverage-matrix extension + Phase
6b JSON / CSV / HTML exports + integration tests; **9b.2**
frontend report page + coverage-matrix code-evidence
rendering + Phase 9b shipped note + coverage gate.

**Design decisions locked in before coding starts:**

- **New `ReportKind::CodeTraceability`.** Kebab
  `"code-traceability"`. Joins the eight existing Phase 6a
  kinds, so the frontend tile grid + Phase 6b export surface
  pick it up automatically.
- **Scanner reuse.** The report calls
  `scan::run_scan(project, world)` per in-scope project
  inside the compute function. Re-walking on each request
  is acceptable at current scale — the coverage-matrix
  report already walks the full mount tree every request.
  A caching layer can land later if latency warrants it;
  the scanner output is already Serialize so a cache key
  would be the scope + world-snapshot id.
- **Response shape.** `CodeTraceabilityReport { scope,
totalArtifacts, uncoveredCount, orphanTagCount, entries,
orphanTags }`. Each entry is
  `{ artifact: CycleNode, expectsCodeTrace: bool,
hasGap: bool, locationsByVerb: BTreeMap<String,
Vec<Location>> }` where `Location = { file, line }`.
  Reusing the Phase 6a `CycleNode` keeps the frontend's
  breadcrumb rendering consistent across reports.
- **Gap semantics.** `hasGap = expectsCodeTrace &&
locationsByVerb.is_empty()`. Matches the spec's "expects
  code trace but for which no matching tag was found".
  `expectsCodeTrace` resolves via the Phase 4 precedence:
  artifact-level `expectsCodeTrace` overrides the
  Collection-level default (which itself defaults to true
  when absent).
- **Coverage-matrix integration is additive.**
  `CoverageParentEntry` grows a new optional
  `coveringCodeEvidence: Vec<CoverageCodeEntry>` field
  where `CoverageCodeEntry = { file, line, verb }`. A
  parent's `hasGap` flag becomes false when either
  `coveringChildren` or `coveringCodeEvidence` is
  non-empty — but only when a code tag's verb matches the
  configured `coveringLinkTypes` set. Verb comparison is
  case-insensitive and uses the Phase 9a canonical form
  (so `"Satisfies"` from a tag matches `"satisfies"` in
  the link-type filter). Existing clients that ignore the
  new field keep working; the matrix page renders the
  annotation as a small `(+N code)` badge.
- **No breaking changes to the unresolved-tag flow.**
  Orphan tags from the scanner come through unchanged in
  the new report's `orphanTags`; they don't leak into the
  coverage matrix (orphans can't cover anything).
- **Exports.** JSON is free via Serialize. CSV emits one
  section for per-artifact locations (artifact, verb,
  file, line) + a second section for orphan tags, matching
  the Phase 6b filesystem-orphans pattern. HTML renders a
  table per artifact with a "gap" badge on uncovered rows.
- **Saved-config shape.** The existing Phase 6a
  `SavedReportConfig { scope, includeInactive, options }`
  round-trips as-is. The code-traceability report has no
  additional per-kind options in 9b; if we add one later
  (e.g. a file-glob exclude) it rides in `options`.
- **Two sub-phases.** Backend changes are roughly the same
  size as adding a new report in Phase 6a, and the frontend
  piece is a single page + a coverage-matrix badge. Two
  sub-phases keeps each commit coherent.

**Tasks:**

1. **`ReportKind::CodeTraceability`** (9b.1) added to the
   Phase 6a enum + `from_kebab` + `as_kebab`. The `/api/
reports/:kind` dispatcher grows an arm.
2. **`compute::code_traceability(scope, query, world)`**
   (9b.1). Runs `run_scan` per in-scope project, groups
   locations by verb per target artifact, derives
   `hasGap`, collects orphan tags.
3. **`CodeTraceabilityReport` DTOs** (9b.1) on
   `reports/mod.rs`. camelCase on the wire; `Location`
   carries just `file` + `line` (the Phase 9a `ScanTag`
   also has `verb` + `rawId` but we've already grouped by
   verb at the entry level).
4. **Coverage-matrix compute update** (9b.1). Build a
   scanner-output index once per matrix request; for each
   parent, look up tags and emit
   `coveringCodeEvidence` entries whose verb is in the
   `coveringLinkTypes` filter. Update `has_gap` logic.
5. **CSV + HTML exports** (9b.1) in `src/exports/{csv,
html}.rs`. Dispatch arm for the new kind; two-section
   CSV (locations + orphans); table-per-artifact HTML with
   a "gap" badge.
6. **Integration tests** (9b.1) end-to-end through the
   axum router: default scope renders expected artifacts,
   orphan tags surface separately, uncovered flag set when
   `expectsCodeTrace` is true and no tag resolves,
   coverage-matrix row shows `coveringCodeEvidence` when
   a tag matches a configured link type, exports in each
   of JSON / CSV / HTML render without schema mismatch.
7. **`CodeTraceabilityReportPage`** (9b.2). New entry in
   `routes/reports/`. Lists artifacts with an expandable
   per-verb locations block, an "uncovered" badge, a
   separate orphan-tag card, and the shared Phase 6a
   `ReportHeader` + `ExportMenu`.
8. **Reports tile-grid entry** (9b.2). Add the new kind to
   the `ReportsIndexPage` tile list so operators discover
   it via the usual navigation.
9. **Coverage-matrix page update** (9b.2). Render
   `(+N code)` badge alongside the existing covering-
   children count when `coveringCodeEvidence` is
   non-empty. Minimal layout change — same row structure,
   just the badge addition.
10. **Frontend types + tests** (9b.2). DTO mirrors on
    `api/types.ts`; a CodeTraceabilityReportPage test
    covering render + uncovered flag + orphan surfacing.
    Coverage-matrix page test extended to assert the
    `(+N code)` badge appears when the backend response
    carries `coveringCodeEvidence`.
11. **Coverage pass + Phase 9b "Shipped" note** (9b.2).
    Rerun `make test` + `make fmt-check` + `make lint`;
    confirm backend line coverage stays ≥ 80 %. Update
    this block with the shipped note per the Phase 7 / 8
    / 9a template.

**Status (Phase 9b):** Shipped. Backend 537 tests (332 unit +
205 integration) / frontend 161 tests green after 9b.1 + 9b.2;
backend line coverage 86.37 % and function coverage 82.95 %,
both up from Phase 9a's 86.29 / 83.03. The code-traceability
report is a first-class report kind at
`/reports/code-traceability` with the usual JSON / CSV / HTML
export surface; the coverage-matrix page shows code-side
evidence alongside artifact-side children.

9b.1 shipped the backend:

- `ReportKind::CodeTraceability` joins the Phase 6a catalog;
  the `/api/reports/:kind` dispatcher grows an arm and the
  Phase 6b export scaffolding picks up JSON / CSV / HTML for
  the new kind at zero additional wiring cost.
- `compute::code_traceability` runs `scan::run_scan` per
  in-scope project, groups locations by Phase 9a canonical
  verb per resolved target, emits a `hasGap` flag
  (`expectsCodeTrace` true AND no tags), and surfaces
  unresolved tags as orphans. The effective
  `expectsCodeTrace` resolver honours the artifact override /
  collection default / true-by-default precedence from
  Phase 4.
- `CoverageParentEntry` grew an additive
  `covering_code_evidence: Vec<CoverageCodeEntry>` field.
  `hasGap` drops to false when either artifact-side children
  or code-side evidence covers the parent under a verb in the
  configured link-type set; existing clients that ignore the
  new field keep working.
- CSV renders a two-section layout (`#### locations` plus
  `#### orphan-tags`) matching the Phase 6b filesystem-
  orphans pattern; uncovered artifacts still emit a row with
  empty verb / file / line columns so the CSV export is a
  complete scoped listing. HTML renders a per-artifact table
  with a "gap" badge on uncovered rows plus a separate
  orphan-tags table.

9b.2 shipped the frontend:

- New `/reports/code-traceability` route with a
  `CodeTraceabilityReportPage` that lists artifacts with
  expandable per-verb location blocks, an "uncovered" badge
  on gaps, and a separate orphan-tag card. Uses the shared
  Phase 6a `ReportHeader` + `ExportMenu` so scope selection
  and exports behave the same as every other report.
- Reports index tile grid picked up the new kind; the copy
  grew from "Seven report classes" to "Nine".
- Coverage-matrix page learned to render the
  `(+N code)` badge and a per-verb file:line list whenever
  the backend response carries `coveringCodeEvidence`. The
  layout change is minimal — the existing row structure is
  preserved and the code list sits beneath the existing
  children list when both are present.

No new risk-adjusted checks were needed for Phase 9b.

### Phase 10a — LLM provider adapters and fallback chain

**Outcome:** Infrastructure only — the adapter layer, fallback
chain, health tracker, and privacy-ack machinery that Phase 10b's
first LLM-dependent feature (rename) will build on. Three
built-in adapter families: OpenAI-compatible, Anthropic, Gemini.
Generic `send_prompt` interface per `LLM-promptAbstraction`; no
feature code yet.

Split into three commit-sized sub-phases on the single branch:
**10a.1** backend config parsing + provider trait + health +
privacy + fallback-chain dispatcher + unit tests;
**10a.2** concrete adapter implementations + HTTP endpoints +
wiremock integration tests; **10a.3** Phase 10a shipped note +
coverage gate.

**Design decisions locked in before coding starts:**

- **Infrastructure only.** No LLM-dependent feature lands in
  10a. Config + adapters + fallback + health + privacy +
  read-only HTTP surface. 10b ships the first feature
  (rename) and — with it — the frontend status/privacy-ack
  UI. Frontend changes in 10a would surface an unused
  provider list.
- **Module layout.** New `src/llm/` with `config.rs` (typed
  parse of `SystemConfig.llm`), `provider.rs` (trait + shared
  error types), `adapters/{openai.rs, anthropic.rs,
gemini.rs}`, `health.rs` (state machine + backoff),
  `chain.rs` (fallback dispatcher), `privacy.rs` (ack
  tracking + local-endpoint detection).
- **Adapter trait.** `async fn send_prompt(&self, prompt:
&str) -> Result<String, AdapterError>` per
  LLM-promptAbstraction. No feature-specific methods.
  `AdapterError` classifies the failure so the chain picks
  the right health-state transition: `Timeout`,
  `RateLimited`, `ServerError`, `Auth`, `ModelNotFound`,
  `Connection`, `Malformed` → hard-disabled via
  Auth/ModelNotFound/Connection/Malformed,
  transient-degraded via Timeout/RateLimited/ServerError.
- **OpenAI-compatible adapter.** Posts Chat Completions to
  `<endpoint>/v1/chat/completions` with `{model, messages:
[{role: "user", content}]}`. No system prompt — feature
  code bakes any context into the user prompt so the adapter
  stays identical across OpenAI, Azure, Ollama, LMStudio,
  vLLM, llama.cpp, OpenRouter, and LiteLLM.
- **Anthropic adapter.** Messages API at `/v1/messages` with
  the native headers (`x-api-key`, `anthropic-version:
2023-06-01`).
- **Gemini adapter.** `generateContent` endpoint with the
  `{contents: [{parts: [{text: prompt}]}]}` shape; key rides
  on the URL per Gemini's convention.
- **Health tracker.** Lives on `AppState` as an
  `Arc<Mutex<HashMap<usize, ProviderHealth>>>` keyed by
  position in the config array. Transient-degraded runs a
  doubling backoff starting at 30 s, capped at 30 min, reset
  on success. Hard-disabled stays until a retest hits it.
  All state in memory only; container restart resets.
- **Privacy acknowledgement.** `HashSet<usize>` on AppState
  recording which provider indices have been ack'd this
  process. Local-endpoint detection treats `localhost`,
  `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`,
  `192.168.0.0/16`, and `[::1]` as local — Ollama / LMStudio
  / llama.cpp default endpoints all fall in this set and
  therefore skip the warning entirely.
- **HTTP surface** (read-only + controls only; no user-
  facing prompt endpoint on the public API):
  - `GET /api/llm/providers` returns `[{index, provider,
model, endpoint, requiresPrivacyAck, isLocal,
apiKeyEnvVar, apiKeyAvailable, health: {status,
retryAfter?}}]`. No secrets in the response.
  - `POST /api/llm/providers/{index}/retest` flips a hard-
    disabled provider through a fresh probe.
  - `POST /api/llm/providers/{index}/acknowledge-privacy`
    records the ack.
  - `POST /api/llm/prompt` (debug, body `{prompt: string}`)
    exercises the chain end-to-end; useful in 10a for
    integration tests + troubleshooting. 10b features hit
    the internal chain directly rather than this endpoint.
- **Secrets handling.** Keys are read from `std::env::var`
  at send time only — never logged, never returned in any
  response. `apiKeyAvailable: bool` in the providers list
  lets the UI surface "configure the env var" hints
  without leaking the value. Per
  LLM-secretsViaEnv the System config never carries a
  literal key.
- **No new deps needed.** `reqwest` (Phase 5b) handles
  provider HTTP; `wiremock` (Phase 5b dev-dep) handles
  simulated-server integration tests.

**Tasks:**

1. **`llm::config`** (10a.1). Typed parse of
   `SystemConfig.llm` into `Vec<ProviderConfig>`; unknown
   provider names → typed error naming the index +
   offending value. Empty / missing array → empty list
   (LLM-optional).
2. **`llm::provider::Adapter` trait + `AdapterError`**
   (10a.1). Generic `send_prompt` signature; error variants
   map to the health-state transitions.
3. **`llm::health`** (10a.1). `HealthState` enum + transition
   rules (healthy → transient-degraded with backoff; any
   state → hard-disabled on Auth/Connection/Malformed; retest
   → healthy/transient/disabled depending on outcome).
   Fully synchronous unit-testable state machine; backoff
   scheduling uses a `Clock` trait so tests can drive
   deterministic transitions without real sleeps.
4. **`llm::privacy`** (10a.1). Local-endpoint detection +
   ack tracker. Unit tests cover each RFC 1918 range plus
   IPv6 loopback.
5. **`llm::chain`** (10a.1). `run_chain(providers, prompt) ->
Result<String, ChainError>`: iterate providers, skip any
   throttled/disabled, invoke adapter, update health on
   outcome, fall through to next on recoverable failure.
   Tests use a trait-object `FakeAdapter` so the pure logic
   is covered without real HTTP.
6. **Concrete adapters — OpenAI-compatible / Anthropic /
   Gemini** (10a.2) in `llm::adapters`. Each builds its
   request body, posts via `reqwest`, parses the response,
   and maps transport-layer errors onto `AdapterError`
   variants.
7. **HTTP endpoints** (10a.2). `GET /api/llm/providers`,
   `POST /api/llm/providers/{index}/retest`, `POST
/api/llm/providers/{index}/acknowledge-privacy`, and the
   debug `POST /api/llm/prompt`. Handlers delegate to
   `llm::chain` and return sanitised JSON.
8. **Integration tests** (10a.2). `wiremock` servers stand
   in for OpenAI / Anthropic / Gemini endpoints; tests cover
   chain success, fallback on 5xx, hard-disable on 401,
   retest flipping a disabled provider back to healthy,
   privacy-ack flow for a cloud endpoint, and the local-
   endpoint bypass.
9. **Coverage pass + Phase 10a "Shipped" note** (10a.3).
   Rerun `make test` + `make fmt-check` + `make lint`;
   confirm backend line coverage stays ≥ 80 %. Update this
   block with the shipped note per the Phase 7 / 8 / 9
   template.

**Status (Phase 10a):** Shipped. Backend 585 tests (370 unit +
215 integration) / frontend 161 tests green after 10a.1 + 10a.2;
backend line coverage 85.58 % and function coverage 83.05 %, both
comfortably over the ≥ 80 % gate (Phase 9b landed at 86.37 /
82.95 — lines drifted down 0.79 pp as expected for a phase that
adds many error-classification branches across three adapter
families, but functions nudged up 0.10 pp). No user-facing
surface ships in 10a by design — the LLM plumbing is in place
and Phase 10b's rename feature (plus the frontend status /
privacy-ack UI) lands on top of it.

10a.1 shipped the infrastructure:

- `llm::config` parses `SystemConfig.llm` into a priority-
  ordered `Vec<ProviderConfig>` per LLM-configSchema; unknown
  provider families surface a typed error naming the offending
  index + value. Empty / missing array → empty list per
  LLM-optional.
- `llm::provider::Adapter` is the single trait every family
  implements — `family()`, `model()`, `endpoint()`,
  `api_key_available()`, and the generic `send_prompt` per
  LLM-promptAbstraction. Dyn-safe via manual `BoxFuture`
  instead of `async_trait`, so no new deps. `AdapterError` is
  split into seven variants whose `kind()` drives the health
  state machine: `Timeout` / `RateLimited` / `ServerError` →
  transient, `Auth` / `ModelNotFound` / `Connection` /
  `Malformed` → permanent.
- `llm::health::HealthTracker` runs the three-state machine
  (`Healthy`, `TransientDegraded { retry_after_secs }`,
  `HardDisabled`) with a 30 s → 30 min doubling backoff,
  keyed by provider-array index. A `Clock` trait makes the
  backoff unit-testable without real sleeps. `should_skip`
  gates the chain; `record_success` / `record_failure` drive
  transitions; `force_healthy` is the retest path that lets an
  operator un-stick a hard-disabled slot.
- `llm::privacy::PrivacyTracker` is a `HashSet<usize>` of
  acknowledged slots; `is_local_endpoint` classifies
  localhost, 127/8, 10/8, 172.16/12, 192.168/16, and ::1 as
  local so Ollama / LMStudio / llama.cpp default endpoints
  skip the privacy warning entirely.
- `llm::chain::run_chain` walks `&[Box<dyn Adapter>]` in
  priority order, honours `should_skip`, calls
  `record_success` / `record_failure` as it goes, and returns
  `(served_by_index, PromptResponse)` on success or a
  `ChainError::AllFailed` carrying per-slot outcomes on
  failure.

10a.2 shipped the concrete adapters + HTTP surface:

- `llm::adapters::openai` implements Chat Completions at
  `<endpoint>/v1/chat/completions` — works interchangeably
  against OpenAI, Azure OpenAI, Ollama, LMStudio, vLLM,
  llama.cpp, OpenRouter, and LiteLLM since all speak the same
  dialect. System prompt is injected as the first
  `{role: "system"}` message when present.
- `llm::adapters::anthropic` posts to `/v1/messages` with
  `x-api-key` and `anthropic-version: 2023-06-01` headers;
  `system` rides as a native top-level field and the response's
  `content` array is flattened to the concatenated text parts.
- `llm::adapters::gemini` posts to `/v1beta/models/{model}:
generateContent` with `{contents, systemInstruction,
generationConfig}` and the API key on the URL per Gemini's
  convention. Assistant turns map to Gemini's `role: "model"`.
- `llm::adapters::common` factors the env-var lookup (per
  LLM-secretsViaEnv, keys are read via `std::env::var` at send
  time only — never logged, never returned) and the
  `reqwest::Error` → `AdapterError` classification so all
  three families stay aligned on timeout / connect / decode
  handling.
- `llm::runtime::LlmRuntime` is the process-lifetime bundle
  of adapters + `HealthTracker` + `PrivacyTracker`. Built once
  on first `publish` from `SystemConfig.llm` and stable
  thereafter; operator reconfiguration requires restart
  (which also resets health and ack state — the locked
  simplification). `run_prompt` walks the chain with both
  health-skip and privacy-gate checks at each slot;
  `retest(index)` force-resets the slot's health then fires a
  tiny "ping" probe whose outcome drives the normal recording
  path.
- HTTP surface, all secret-safe: `GET /api/llm/providers`
  returns `{index, provider, model, endpoint, isLocal,
requiresPrivacyAck, apiKeyEnvVar, apiKeyAvailable, health}`
  per slot — the env-var _name_ is surfaced so operators can
  pinpoint the setting, but the value never leaves the
  server. `POST /api/llm/providers/{i}/retest`,
  `POST .../acknowledge-privacy`, and `POST /api/llm/prompt`
  (the debug endpoint — 10b features will call the chain
  directly rather than going through HTTP).
- `tests/llm_adapters.rs` drives the full surface through a
  `wiremock::MockServer` per family: chain success for each
  of the three families, fallback on 5xx, hard-disable on 401,
  retest flipping a hard-disabled slot back to healthy after
  the mock stops 401-ing, privacy-ack gate blocking then
  clearing for a cloud endpoint, local-endpoint bypass, the
  providers-list shape (with `apiKeyAvailable` reflecting a
  deliberately-unset env var), and the empty-config path.
- Workspace added only the `json` feature to the existing
  `reqwest` dep — no new crates.

### Phase 10b — LLM-assisted rename

**Outcome.** Two first LLM-consuming features building on 10a's
chain:

1. **Rename suggestion in the rename dialog** (per
   `LLM-renameWorkflow`). When the chain has at least one healthy +
   ack'd provider, the existing rename dialog gains a "Suggest
   names" button. Clicking calls a new backend endpoint that runs
   the chain and returns three candidate stems + one-line
   rationales; clicking a candidate fills the text field. The
   manual text field stays — the LLM is additive, and per
   `LLM-optional` the dialog behaves identically when no providers
   are configured.
2. **Post-doorstop-import rename wizard** (per
   `LLM-postImportRenameSuggest`). After a successful import, the
   report page gains a "Suggest names" action opening a modal that
   runs suggestions across every imported artifact, shows a
   before/after table with per-row checkboxes, and applies the
   selected renames with one button. Uses the existing single-
   artifact rename endpoint under the hood.

The sub-phase split mirrors 10a: **10b.1** backend endpoints +
wiremock tests; **10b.2** frontend UI (rename-dialog button,
providers status page, post-import wizard) + Vitest coverage;
**10b.3** coverage pass + Phase 10b shipped note.

**Locked decisions:**

- **Scope.** Backend endpoints + frontend UI + a new `/llm`
  providers page. No schema change, no new deps (Phase 10a already
  pulled in the `json` reqwest feature; everything else is
  existing). Phase 10c (MCP) is independent of 10b.
- **Suggestion count: 3.** Fewer is terse, more clutters the UI.
  The prompt instructs the model to return exactly three.
- **Prompt construction.** Single `PromptRequest` with `system` =
  naming-convention rules (the collection's prefix + the existing
  filename stems in the collection as style anchors) and `user` =
  the current title + the current name + a one-line ask. The
  response instruction is "exactly three names, newline-separated,
  in `name — rationale` format." `max_tokens: 200`,
  `temperature: 0.2`.
- **Response parsing.** Server parses newline-separated
  `name — rationale` lines, validates each stem against the
  existing filename regex (`[A-Za-z0-9._\-]+`), drops invalid
  ones, deduplicates against the current name, and errors with a
  typed `NoSuggestions` if zero survive. Best-effort feature —
  never block a rename because the LLM was vague.
- **Plain fallback stays the primary path.** Per INTENTIONS.md —
  "features with plain equivalents (e.g. rename) fall back to the
  plain action" — the rename dialog's manual text field is
  untouched. The Suggest button is hidden when
  `/api/llm/providers` returns an empty list.
- **Privacy gate.** The new rename-suggestions endpoints run
  through `LlmRuntime::run_prompt`, which already enforces the
  privacy-ack gate from Phase 10a. When every eligible provider
  requires ack, the endpoint returns a structured
  `privacyAckRequiredFor: [indices]` arm so the UI can route
  operators to the right "Acknowledge privacy" button.
- **Bulk endpoint.** `POST
/api/projects/{slug}/rename-suggestions/bulk` takes
  `{ uuids: [...] }`, fans out with bounded concurrency via
  `tokio::task::JoinSet` (parallelism 4), and returns
  `{ suggestions: { [uuid]: [{name, rationale}] } }`. The caller
  decides which to apply, then loops the existing
  `PATCH /api/artifacts/{uuid}` rename. No new atomic "apply all"
  endpoint — each rename remains an individual atomic write per
  `STOR-atomicWrites`, and a failure on one artifact doesn't
  block the rest.
- **Providers status page at `/llm`.** A sidebar entry appears
  only when `/api/llm/providers` returns at least one entry. The
  page lists every provider with its current health state,
  endpoint, `apiKeyEnvVar` + `apiKeyAvailable` flag, a **Retest**
  button (always visible), and an **Acknowledge privacy** button
  (visible only when `requiresPrivacyAck: true`). No new backend
  work — purely the 10a endpoints.
- **Doorstop-import hook is additive.** The import-report page
  gets a "Suggest names" button that opens the wizard modal; the
  report is complete without it, so importing remains a self-
  contained operation. When LLM is not configured, the button is
  hidden.
- **Tests.** Backend: wiremock integration tests covering chain
  success at the handler layer (one family is enough — the
  adapter layer already has per-family coverage from 10a.2),
  malformed LLM output → typed error, bulk-endpoint concurrency,
  privacy-ack blocking, the no-LLM path returning 200 with an
  empty list. Frontend: Vitest + React Testing Library for the
  Suggest-names button flow, the post-import wizard, and the
  `/llm` page (happy path + retest + ack-required + unset-env-var
  hint).
- **No new deps.** Reuses `reqwest` / `wiremock` / `tokio` /
  `@tanstack/react-query` / Tailwind.

**Tasks:**

1. **`POST /api/artifacts/{uuid}/rename-suggestions`** (10b.1).
   New module `rename_suggest.rs` with the prompt builder +
   response parser; handler delegates to `LlmRuntime::run_prompt`;
   DTO carries three suggestions + `servedBy` + an optional
   `privacyAckRequiredFor: [indices]` arm for the ack-required
   case.
2. **`POST /api/projects/{slug}/rename-suggestions/bulk`**
   (10b.1). Same engine, fanned out with `tokio::task::JoinSet`
   at concurrency 4. Returns a map keyed by UUID.
3. **Backend wiremock tests** (10b.1). Chain success, malformed
   output, privacy-ack block, bulk concurrency, empty-config path.
4. **Frontend `/llm` providers page** (10b.2). New route, lists
   providers via `GET /api/llm/providers`, per-row Retest +
   Acknowledge buttons with react-query invalidation on success,
   env-var-missing hints.
5. **Frontend Suggest-names in the rename dialog** (10b.2).
   Button + loading state + result list; clicking a candidate
   fills the text input. Hidden when no providers are configured.
6. **Frontend post-doorstop-import rename wizard** (10b.2). New
   button on the import-report page; modal opens, runs bulk
   endpoint, shows table with checkboxes, applies selected
   renames serially via the existing PATCH rename endpoint.
7. **Frontend tests** (10b.2). Vitest covering the three UI
   surfaces using the existing `stubFetchByPath` pattern.
8. **Coverage pass + Phase 10b "Shipped" note** (10b.3). Rerun
   `make test` + `make fmt-check` + `make lint`; confirm backend
   line coverage stays ≥ 80 %. Update this block with the shipped
   note per the Phase 7 / 8 / 9 / 10a template.

**Status (Phase 10b):** Shipped. Backend 607 tests (384 unit + 223
integration) / frontend 172 tests green after 10b.1 + 10b.2; backend
line coverage 85.68 % and function coverage 83.53 %, both comfortably
over the ≥ 80 % gate and up from Phase 10a's 85.58 / 83.05. Two
first-class LLM-consuming features landed on top of 10a's adapter
layer: the rename dialog gained a Suggest-names button, and the
doorstop import flow gained a post-import rename wizard.

10b.1 shipped the backend:

- `rename_suggest::build_prompt` produces a generic `PromptRequest`
  carrying the collection prefix, up to eight sibling stems as
  style anchors, the current title, and a fixed "exactly three
  suggestions, name — rationale format" instruction. `max_tokens:
200`, `temperature: 0.2` per the ROADMAP locked decision.
- `rename_suggest::parse_suggestions` accepts em-dash, hyphen, or
  colon separators; strips numbered-list / bullet prefixes;
  validates each stem against the existing filename regex
  (`[A-Za-z0-9._\-]+`), caps lengths at 64 characters, dedupes
  against the current name + against itself; caps at three
  suggestions; surfaces a typed `NoSuggestions` error when
  nothing survives so the UI can degrade gracefully (renaming
  without the LLM still works).
- `POST /api/artifacts/{uuid}/rename-suggestions` — three-arm
  response enum (`ok` / `privacyAckRequired` / `noProviders`).
  The privacy-ack arm carries the provider indices the UI should
  ask the operator to acknowledge, so the rename dialog can route
  the user to the right `/llm` button directly.
- `POST /api/projects/{slug}/rename-suggestions/bulk` — fans out
  with `tokio::task::JoinSet` and a `tokio::sync::Semaphore(4)`,
  returns per-UUID outcomes in a mixed-kind result list
  (`ok` / `error` / `privacyAckRequired` / `notFound`). A single
  failed artifact doesn't abort the bulk run — the post-import
  wizard can render a partial table and apply only the rows that
  succeeded.
- 14 pure-compute unit tests for the prompt shape + parser cases;
  8 wiremock integration tests covering happy path, malformed LLM
  output → typed BAD_GATEWAY error, unacked cloud provider →
  `privacyAckRequired`, empty-LLM-config → `noProviders`, bulk
  per-UUID results, bulk `notFound` for stale UUIDs, bulk
  concurrency verified by measuring wall-clock (<600 ms for three
  300 ms mock requests — serial would be ~900 ms), bulk no-LLM
  returns empty results. No new dependencies — reused the
  `reqwest` / `wiremock` stack from 10a and the existing
  `tokio::sync::Semaphore`.

10b.2 shipped the frontend:

- `/llm` providers page — new route listing every configured
  provider with its health badge (`healthy` /
  `transient-degraded` / `hard-disabled`), endpoint, env-var name,
  `apiKeyAvailable` flag (with an "env var is unset" badge when
  false), a Retest button (always visible, re-probes the slot),
  and an Acknowledge-privacy button (visible only when
  `requiresPrivacyAck: true`). Sidebar entry hides itself when
  `/api/llm/providers` returns an empty list, so the `LLM-optional`
  baseline UX is unchanged for operators without LLM.
- Suggest-names button in the artifact rename dialog — hidden
  when no LLM is configured; on click calls the
  rename-suggestions endpoint, renders three clickable suggestion
  pills that fill the name field, and handles the
  `privacyAckRequired` arm with an inline alert linking to `/llm`.
  The manual text field stays the primary path; the plain-rename
  flow is untouched.
- Post-doorstop-import rename wizard — the import-report panel
  gains a Suggest-names button (only when LLM is configured)
  that opens a modal fetching every imported artifact across the
  imported collections, calling the bulk rename-suggestions
  endpoint, and letting the operator pick one suggestion per row.
  Applying runs the picks serially through the existing PATCH
  rename endpoint, so a single failure doesn't abort the rest
  and per-row errors surface inline.
- API layer: typed `LlmProviderEntry` / `LlmHealthState` /
  `RenameSuggestion` / `RenameSuggestionsResponse` /
  `BulkRenameSuggestionEntry` definitions; five new fetch
  wrappers; five new react-query hooks with invalidation on
  retest + ack so the `/llm` page refreshes after operator action.
- 11 new Vitest files covering /llm (empty state, provider list
  shape with health / env-var / ack columns, retest POST round-
  trip, ack POST round-trip), the rename dialog (Suggest panel
  hidden when no LLM, suggestion list renders, clicking a pill
  fills the input, privacy-ack arm links to `/llm`), and the
  post-import wizard (fetch artifacts → bulk suggest → pick →
  apply PATCH renames, privacy-ack arm surfaces a link to
  `/llm`). No new dependencies.

### Phase 10c — MCP server for AI coding agents

**Outcome.** A standalone `reqforge-mcp` binary that speaks the
Model Context Protocol over stdio, turning ReqForge's artifacts /
links / reports / reviews into first-class context for Claude
Code, Cursor, Zed, GitHub Copilot, and similar agents. Implements
the read-only surface described in `LLM-mcpServer` /
`LLM-mcpTools` / `LLM-mcpResources` / `LLM-mcpPrompts`. AI-driven
writes (`LLM-mcpReadWrite`) stay deferred until the read-only
surface is proven in practice.

The sub-phase split mirrors 10a and 10b: **10c.1** crate scaffold +
stdio transport + JSON-RPC framing + eleven read-only tools +
wiremock unit tests; **10c.2** resources surface + canned prompts +
end-to-end test against a real in-process reqforge-server;
**10c.3** client-wiring docs + Phase 10c shipped note.

**Locked decisions:**

- **Separate binary, thin adapter.** A new workspace member
  `backend/reqforge-mcp` with its own `main.rs`. It speaks MCP on
  stdio and makes HTTP calls into a running `reqforge-server` on
  `--url` (default `http://127.0.0.1:36743`). Matches
  INTENTIONS.md's "thin adapter over the REST API" framing. No
  changes to `reqforge-server` itself in 10c — MCP only consumes
  endpoints that already exist.
- **Stdio transport only.** Every AI coding agent on the market
  today (Claude Code, Cursor, Zed, Copilot) supports stdio;
  Streamable HTTP is more complex and uncommon. Defer HTTP / SSE.
- **JSON-RPC 2.0 hand-rolled over serde_json.** No new deps. The
  protocol is ~200 lines of framing + dispatch; pulling in an MCP
  SDK would be comparable code for another crate dependency to
  track.
- **Read-only surface.** No tools that mutate state. The existing
  read-only REST endpoints cover the `LLM-mcpTools` categories.
- **Tool set.** Eleven tools mapping directly to existing REST
  endpoints: `reqforge_list_projects`, `reqforge_get_project`,
  `reqforge_list_collections`, `reqforge_get_collection`,
  `reqforge_list_artifacts`, `reqforge_get_artifact`,
  `reqforge_get_artifact_by_path`, `reqforge_get_incoming_links`,
  `reqforge_search`, `reqforge_run_report`, `reqforge_get_graph`.
  Each takes JSON-schema-typed arguments and returns MCP text-
  content blocks.
- **Resources per artifact.** `resources/list` returns every
  artifact in the System as a `reqforge://artifact/{uuid}`
  resource with a human-readable name
  (`{slug}/{prefix}/{artifactName}`). `resources/read` fetches
  the artifact body. Flat list in 10c — pagination lands if a
  concrete System size trips the agent-side UI; MCP spec
  supports cursor-based pagination when needed.
- **Canned prompts.** Six workflow prompts covering the list
  from INTENTIONS.md: `gap_analysis`, `coverage_summary`,
  `review_assist`, `implementation_planning`,
  `test_gap_planning`, `impact_analysis_narrative`. Each takes
  optional arguments (scope, seed UUID) and returns a `messages`
  array the agent fills in.
- **Privacy + localhost.** The MCP server connects to
  `127.0.0.1:36743` by default. Non-loopback URLs are permitted
  but gated behind a `--allow-remote` flag so an operator doesn't
  accidentally expose requirements to a remote ReqForge instance.
  Matches the `LLM-privacyWarning` principle applied to the MCP
  direction of travel.
- **No auth.** Consistent with ReqForge's single-user-localhost
  posture from INTENTIONS.md. The agent and ReqForge run on the
  same host; whoever can reach ReqForge can also read
  requirements directly.
- **Tests.** Unit tests for JSON-RPC framing, request / response
  serialization, and each tool / prompt / resource handler
  against a `wiremock` stand-in for `reqforge-server`.
  End-to-end integration test that spawns the mcp binary as a
  child process, drives it via stdin / stdout, and asserts the
  full `initialize → tools/list → tools/call → resources/list →
resources/read → prompts/get` round-trip.
- **Docs.** Short `docs/mcp.md` guide showing how to wire
  `reqforge-mcp` into Claude Code
  (`~/.config/claude/claude.json`), Cursor, and Zed. README.md
  picks up a "Use with AI coding agents" subsection pointing at
  it.

**Tasks:**

1. **Workspace member + stdio transport** (10c.1). New crate
   `backend/reqforge-mcp`, `Cargo.toml` pulling only workspace-
   shared deps (tokio, serde, serde_json, reqwest, clap,
   tracing). Implements the JSON-RPC 2.0 framing (newline-
   delimited JSON over stdio) + dispatcher that routes `method`
   names to handlers.
2. **`initialize` handshake + capability declaration** (10c.1).
   Responds with protocol version, server info, and the
   capability map (`tools`, `resources`, `prompts`). `ping` for
   keepalive.
3. **Eleven read-only tools** (10c.1). Each tool handler calls
   the corresponding REST endpoint via `reqwest`, converts the
   JSON response to an MCP text-content block, and maps HTTP
   errors to MCP error responses.
4. **Wiremock unit tests** (10c.1). Per-tool tests that stub the
   relevant REST endpoint and assert the MCP response shape +
   HTTP error propagation.
5. **Resources surface** (10c.2). `resources/list` walks
   projects → collections → artifacts to emit
   `reqforge://artifact/{uuid}` URIs. `resources/read` fetches
   the artifact body.
6. **Prompts surface** (10c.2). Six prompt templates following
   the `LLM-mcpPrompts` list. Each returns a `PromptMessage`
   array with the system + user templates the agent fills in.
7. **End-to-end integration test** (10c.2). Spawns
   `reqforge-mcp` as a child process, drives stdin / stdout, and
   runs the full `initialize → tools/list → tools/call →
resources/list → resources/read → prompts/get` flow against a
   real in-process `reqforge-server` built via the existing
   tempfile harness.
8. **Client wiring docs** (10c.3). New `docs/mcp.md` with copy-
   pasteable snippets for Claude Code, Cursor, and Zed. README.md
   gains a "Use with AI coding agents" subsection pointing at it.
9. **Coverage pass + Phase 10c "Shipped" note** (10c.3). Rerun
   `make test` + `make fmt-check` + `make lint`; confirm backend
   line coverage stays ≥ 80 %. Update this block with the shipped
   note per the Phase 7 / 8 / 9 / 10a / 10b template.

**Status (Phase 10c):** Shipped. Backend 657 tests (434 unit +
223 integration) / frontend 172 tests green after 10c.1 + 10c.2;
backend line coverage 85.67 % and function coverage 83.04 %, both
comfortably over the ≥ 80 % gate (lines essentially flat vs
10b's 85.68 %; functions down 0.49 pp from 83.53 % as expected
for a phase that lands a whole new crate of small handlers).
AI coding agents can now query ReqForge as first-class context
instead of parsing raw files — the `reqforge-mcp` binary ships
eleven read-only tools, one resource per artifact, and six
canned workflow prompts over stdio JSON-RPC.

10c.1 shipped the backend scaffold + transport + tools:

- New workspace member `backend/reqforge-mcp`, a standalone
  binary speaking MCP (JSON-RPC 2.0) on stdio. `Cargo.toml` pulls
  only workspace-shared deps (tokio, serde, serde_json,
  reqwest, url, thiserror, tracing) — no new crates on the
  workspace dep graph.
- `protocol` — hand-rolled JSON-RPC 2.0 + MCP types: requests,
  responses, capability maps, tool definitions, content blocks.
  Protocol version pinned to `2024-11-05`.
- `client` — reqwest-based GET-only client over
  `reqforge-server`'s REST API with 15 s timeout and HTTP-error
  propagation that includes a 500-byte body preview so
  operators can diagnose from the agent's tool output.
- `tools` — eleven handlers mapping one-to-one to existing REST
  endpoints: `list_projects`, `get_project`, `list_collections`,
  `get_collection`, `list_artifacts`, `get_artifact`,
  `get_artifact_by_path`, `get_incoming_links`, `search`,
  `run_report`, `get_graph`. Each takes JSON-schema-typed
  arguments and returns MCP text-content blocks.
- `transport` — stdio newline-delimited JSON read/write loop +
  dispatcher routing `initialize` / `ping` / `tools/list` /
  `tools/call` and (post-10c.2) `resources/*` / `prompts/*`.
  Upstream tool failures surface as `CallToolResult {
isError: true }` per MCP convention; protocol-level errors
  stay JSON-RPC error responses with the right error code.
- `main` — hand-parsed `--url` / `--allow-remote` /
  `--help` / `--version` args (no clap dep). Loopback gate
  refuses non-local URLs unless `--allow-remote` is passed,
  applying the `LLM-privacyWarning` principle to the MCP
  direction of travel. Tracing goes to stderr so stdout stays
  reserved for JSON-RPC.

10c.2 shipped the resources + prompts + end-to-end round-trip:

- `resources` — `resources/list` walks projects → collections →
  artifacts and emits one `reqforge://artifact/{uuid}` URI per
  artifact with a human-readable `{slug}/{prefix}/{name}`
  breadcrumb name. `resources/read` parses the URI, fetches the
  artifact, and renders a self-contained markdown document
  (title, path, UUID, shape, tags, body) the agent can drop
  straight into its context window.
- `prompts` — six canned workflow templates per
  `LLM-mcpPrompts`: `gap_analysis`, `coverage_summary`,
  `review_assist`, `implementation_planning`,
  `test_gap_planning`, `impact_analysis_narrative`. Each
  injects the optional `scope` / required `uuid` into the body
  and instructs the agent on which `reqforge_*` tools to call
  and how to structure its response. Pure compute — no HTTP
  calls.
- `transport` — dispatcher routes `resources/list`,
  `resources/read`, `prompts/list`, `prompts/get` through the
  new modules; `initialize` advertises the expanded capability
  map with `subscribe: false` and `listChanged: false` since
  10c doesn't push update notifications (agents poll
  `resources/list` when they want a fresh snapshot).
- `tests/end_to_end.rs` — spawns the compiled `reqforge-mcp`
  binary as a child process, drives stdin / stdout with
  JSON-RPC, and asserts the full `initialize → tools/list →
tools/call → resources/list → resources/read → prompts/list →
prompts/get → ping → stdin-close-triggers-exit` round-trip
  against a `wiremock` stand-in for `reqforge-server`. Using
  wiremock keeps `reqforge-server` out of the mcp crate's dep
  graph, consistent with the standalone-binary locked decision
  — the child process sees identical HTTP shapes either way.

10c.3 shipped the docs + coverage:

- New `docs/mcp.md` with copy-pasteable config snippets for
  Claude Code (`~/.config/claude-desktop/claude_desktop_config.json`),
  Cursor (`~/.cursor/mcp.json`), and Zed
  (`~/.config/zed/settings.json`). Explains the tool / resource /
  prompt surface, the `--url` / `--allow-remote` flags, privacy
  posture, an example agent session, and troubleshooting.
- README.md gains a "Use with AI coding agents" subsection
  pointing at it.
- Gates all green at the phase boundary: `make test` (657
  backend + 172 frontend), `make fmt-check`, `make lint`, and
  `cargo clippy -D warnings` across the full workspace.

### Phase 11a — Schema migration

**Outcome.** The schema-migration machinery described across every
`STOR-schema*` spec, ready for the first real schema bump. Today
the current version is `1` for all four file types (artifact
frontmatter, collection config, project config, system config)
and no migration steps are registered, so the user-visible
behaviour of this phase is minimal — but the load-path plumbing,
the bulk-migrate HTTP endpoint, and the UI action all exist and
can be exercised against a synthesized "newer" file to prove the
guard rails hold.

The sub-phase split mirrors prior LLM phases: **11a.1** backend
infrastructure (registry, load-path integration, bulk-migrate
endpoint) + backend tests; **11a.2** frontend UI (migrate button,
modal, schema-diagnostic banner) + Vitest; **11a.3** coverage pass

- Phase 11a shipped note.

**Locked decisions:**

- **Per-file-type registry.** One migration chain per file type,
  compile-time declared. `FileType` enum: `Artifact`, `Project`,
  `Collection`, `System`. Current version is `1` for every type
  today. Adding a migration means editing the source — no runtime
  registration.
- **Migrations operate on `serde_json::Value`.** Each step:
  `fn(Value) -> Result<Value, MigrationError>`. No typed
  intermediate representations — we can't carry old typed structs
  indefinitely, and the JSON-native on-disk format already hands
  us `Value` before typed deserialization.
- **Fallible.** A migration that errors aborts the operation
  (load or bulk-migrate) with a typed error naming the step and
  the file. Never writes a partial result.
- **Load-path integration.** Every JSON / YAML loader
  (`frontmatter.rs`, the readers for Project / Collection / System
  configs) is rewrapped to call `migrate_value(file_type, raw)`
  before typed deserialization. No change for `v == current`.
  Returns `NewerThanCurrent` for `v > current`.
- **Refuse-newer surfaces as a project-level diagnostic** per
  `STOR-schemaNewerFilesRefused`. The offending file doesn't load;
  its containing project loads with a new `schemaDiagnostics`
  field naming the file + versions. The frontend shows a banner
  on the project; CRUD on the offending artifact is blocked;
  other artifacts in the project keep working. No server-wide
  crash.
- **Bulk-migrate HTTP endpoint.** `POST
/api/projects/{slug}/migrate-schema { force?: bool }`. Walks the
  project config + every collection config + every artifact
  frontmatter, applies migrations, and rewrites each changed file
  via the existing atomic-write path. Response: per-file
  before / after versions + totals. Today every call is a no-op
  (every file is `v=1`), but the endpoint exists so
  `STOR-schemaBulkMigrate` is honored.
- **Uncommitted-changes pre-flight** via `gix` (already a
  workspace dep). Default `force=false` refuses the run with a
  `409` if the project's worktree is dirty so the migration lands
  as its own commit. `force=true` overrides for operators who
  know what they're doing. ReqForge never commits — the operator
  does.
- **Lazy write-back stays.** Load-time migrations produce migrated
  in-memory values only; the on-disk `schemaVersion` bumps when
  the user edits the file (or runs bulk-migrate). Matches
  `STOR-schemaLazyWriteBack`.
- **No CLI.** `STOR-schemaCliMigrate` defers with the broader CLI
  / headless deferral per INTENTIONS.md. The HTTP endpoint covers
  the UI path.
- **System config is out of scope for bulk-migrate.** It's not
  inside any one project; a separate endpoint can land if and
  when a system-config migration is actually needed.
  `STOR-schemaBulkMigrate` reads as "per-Project", so this is in
  spec.
- **No new deps.** `serde_json` / `gix` / `reqwest` / `axum` all
  already in the workspace.

**Tasks:**

1. **`schema_migration` module** (11a.1). New
   `src/schema_migration/` with `registry.rs` (FileType enum +
   MigrationStep + Registry), `errors.rs` (SchemaMigrationError),
   and `artifact.rs` / `collection.rs` / `project.rs` /
   `system.rs` each declaring an empty chain.
   `migrate_value(file_type, raw) -> Result<(Value,
MigrationOutcome), SchemaMigrationError>` is the public API.
2. **Load-path integration** (11a.1). Wrap
   `frontmatter::parse_artifact_frontmatter` and the project /
   collection / system config readers so every raw JSON / YAML
   goes through `migrate_value` before typed deserialization.
3. **Per-project diagnostics** (11a.1). New `SchemaDiagnostic`
   entry on `LoadedProject::diagnostics` when a file is too new.
   CRUD handlers refuse writes on the offending file with a
   typed `409`.
4. **Bulk-migrate endpoint** (11a.1). `POST
/api/projects/{slug}/migrate-schema` with the gix uncommitted-
   changes check, per-file walk, atomic rewrites, and a
   structured per-file result list.
5. **Backend unit + integration tests** (11a.1). Registry engine
   tests against a test-only `FileType::__Test` with a 2-step
   chain; integration tests covering the bulk endpoint happy
   path (all-v1 → `0` rewrites), the too-new-refusal path, and
   the dirty-worktree `409`.
6. **Frontend migrate-schema UI** (11a.2). New "Migrate schema"
   button on the project detail page, confirmation modal (shows
   what will be walked + pre-flight warnings), result display.
   Schema-diagnostic banner at the top of the project detail
   when any file is too new.
7. **Frontend Vitest** (11a.2). Happy path (all-v1 → "0 files
   migrated"), blocked-by-dirty-worktree arm, schema-diagnostic
   banner render.
8. **Coverage pass + Phase 11a "Shipped" note** (11a.3). Rerun
   `make test` + `make fmt-check` + `make lint`; confirm backend
   line coverage stays ≥ 80 %. Update this block with the shipped
   note per the Phase 7 / 8 / 9 / 10a / 10b / 10c template.

**Status (Phase 11a):** Shipped. Backend 679 tests (456 unit + 223
integration) / frontend 178 tests green after 11a.1 + 11a.2;
backend line coverage 84.88 % and function coverage 82.44 %, both
comfortably over the ≥ 80 % gate (lines down 0.79 pp vs 10c's
85.67 % — a whole new module of error-path branches landed
alongside a small test set; functions -0.60 pp for the same
reason). The schema-migration machinery is in place for the first
real schema bump: today every registered chain is empty, so the
observable user surface is the guard rails (too-new refusal) and
the migrate-schema action (a no-op on a v1-clean project). The
first `fn(Value) -> Result<Value>` step added to any chain lands
on a single module, and both the load path and bulk-migrate
endpoint pick it up automatically.

11a.1 shipped the backend infrastructure:

- New `src/schema_migration/` module. `registry.rs` declares a
  `FileType` enum (`Artifact` / `Collection` / `Project` /
  `System`) and a `Registry` chain engine parameterised on
  `base_version` / `current_version` / fallible `MigrationStep`
  functions. `Registry::migrate` walks from the file's declared
  version up to current, refuses too-new files, propagates step
  failures with `from → to` detail, and double-stamps
  `schemaVersion` after each step so a buggy migration can't
  silently produce the wrong value. `errors.rs` carries a typed
  `SchemaMigrationError { InvalidSchemaVersion, NewerThanCurrent,
StepFailed }` that both loaders and the bulk engine surface.
- Per-file-type chain modules (`artifact.rs`, `collection.rs`,
  `project.rs`, `system.rs`) — empty chains today. The first real
  bump edits one module's `STEPS` array and the top-level
  `CURRENT_*_VERSION` constant; nothing else needs to change.
- `bulk.rs` walks the project config + every collection config +
  every artifact frontmatter + every blob / URL sidecar,
  atomic-rewrites any migrated file via the existing
  `write::atomic_write` path, and returns a per-file result list.
  The dirty-worktree pre-flight shells out to
  `git status --porcelain` since the gix feature set we link
  against doesn't expose a fully-baked worktree-status iterator at
  this version; presence of any output means dirty. `force=true`
  bypasses the check.
- Load-path integration: `load/artifact.rs`, `load/blob.rs`,
  `load/url.rs`, `load/project.rs`, `system.rs` thread raw
  `serde_json::Value` through `migrate_value` before typed
  deserialization. Each gained a `Schema` error variant + a
  `schema_too_new()` accessor that returns `(found, current)` so
  the project walker emits a dedicated
  `LoadDiagnostic::SchemaTooNew` alongside the generic
  `ArtifactFailed` / `CollectionConfigInvalid`. The offending file
  doesn't load; the containing project stays usable for every
  other file.
- `ProjectDetail` grows a `schemaDiagnostics: []` field serialized
  from `SchemaTooNew` diagnostics, omitted from the wire when
  empty so v1-clean projects keep the existing shape per
  `STOR-schemaNewerFilesRefused`.
- `POST /api/projects/{slug}/migrate-schema { force? }` — returns
  `200` with the per-file result on success, `409` on dirty
  worktree, `404` on unknown slug. Runs via `spawn_blocking` so
  the walk + atomic writes don't pin the axum executor.
- 17 new unit tests (registry engine + bulk walker) + 5
  integration tests for the HTTP surface (all-v1 happy path,
  unknown-slug 404, too-new artifact failure entries,
  `schemaDiagnostics` surfacing in `ProjectDetail`, clean-project
  wire shape).

11a.2 shipped the frontend:

- Project-detail page gains a "Migrate schema" button next to
  "Import from doorstop" and "New collection".
- `MigrateSchemaDialog` — confirmation modal that posts
  `{ force: false }` by default. A `409` response flips the
  modal into a dirty-worktree warning with a distinct amber
  "Run anyway" button that re-posts with `force: true`. After
  success, a result panel shows scanned / rewritten / up-to-date /
  failure counts, a collapsed rewrite list, and an expanded
  failure list.
- `SchemaDiagnosticsBanner` — alert surface at the top of the
  project detail page listing per-file `schemaVersion` /
  `fileType` / found-vs-current versions. Renders nothing when
  `schemaDiagnostics` is empty so v1-clean projects are
  unaffected.
- API layer: typed `SchemaDiagnostic` / `BulkMigrateResult` /
  `MigrateSchemaRequest` / `MigrateSchemaResponse`; new fetch
  wrapper + react-query hook that invalidates all caches on a
  successful run (the rewrites bumped file contents, so every
  stale cache needs a refresh).
- 6 new Vitest files (dialog: force-false default happy path,
  dirty-worktree 409 arm + Run-anyway re-post with `force=true`,
  per-file failures render; banner: empty-list renders nothing,
  multi-entry shows versions, singular copy for exactly one
  diagnostic).

### Phase 11b — Sample content onboarding

**Outcome.** The "Create sample content" choice from
`UX-initSampleContent` — a one-click action that populates a just-
initialised project with a small, realistic set of Collections +
artifacts so a new user can see traceability, reports, and the
graph in action before writing anything themselves.

The sub-phase split mirrors prior phases: **11b.1** backend
generator + endpoint + tests; **11b.2** frontend post-init choice
screen + empty-state button + Vitest; **11b.3** coverage pass +
shipped note.

**Locked decisions:**

- **Fixed, compiled-in sample set.** Hand-written demo artifacts
  shipped with the binary — no runtime customization, no template
  engine. A single `fn generate() -> Vec<CollectionDraft>`
  returns the deterministic set the generator writes. Unit tests
  assert the shape; future changes are a one-file diff.
- **Scenario: "Task Tracker"** — a mini requirements-for-a-todo-
  app set that's immediately legible to the ReqForge target
  audience. Three collections (REQ, DES, UC), ~7 artifacts
  total, all content-hosted. No blob / URL samples in 11b — those
  need binary content and their UX value is incremental.
- **Link coverage.** The sample uses `satisfies`,
  `derives-from`, and `verifies` to exercise the graph + matrix
  - coverage-matrix reports meaningfully. Every artifact has at
    least one incoming or outgoing link.
- **Atomic generation, refuse-if-non-empty.** `POST
/api/projects/{slug}/sample-content` returns `201` on success,
  `409` if the project already has any collection (protects real
  work). No `force` flag — the spec is "starter content for an
  empty project"; re-seeding isn't the use case.
- **Backend generator is pure-compute.** Returns structured
  drafts; the HTTP handler wraps in the existing
  `CreateCollection` / `CreateArtifact` write paths so sample
  content goes through the same atomic-write + ownership-
  reconcile stack as hand-created artifacts. No new write
  surface.
- **Post-init choice screen in the init wizard.** After
  `POST /api/mounts/{dir}/init` succeeds, `InitProjectDialog`
  switches from its form to a three-button choice panel: **Start
  empty** (current behaviour — navigate to empty project),
  **Create sample content** (post to the new endpoint then
  navigate to the project), **Import from doorstop** (navigate +
  open the existing doorstop-import dialog; always shown — the
  import dialog handles "no markers" gracefully). Matches
  `UX-postInitChoice` at a minimum — full marker detection can
  stay deferred since the doorstop dialog already handles it.
- **Empty-state fallback.** The existing `EmptyCollectionsState`
  (shown on a project detail page with no collections) gains a
  "Create sample content" action alongside "New collection" —
  operators who skipped the post-init choice can still seed
  sample content later.
- **Tests.** Backend unit tests cover the generator (artifact
  count, link-target resolution, UUIDs are valid v7). Integration
  tests cover the endpoint (writes files to a real tempfile
  project, returns `409` on a non-empty project, returns `404`
  for unknown slug). Frontend Vitest covers the post-init choice
  flow (clicking "Create sample content" posts + navigates;
  "Start empty" just navigates; "Import from doorstop" opens the
  doorstop dialog) and the `EmptyCollectionsState` button.
- **No new dependencies.**

**Tasks:**

1. **`sample_content` module** (11b.1). New
   `src/sample_content.rs` with `CollectionDraft` / `ArtifactDraft`
   shapes + a `generate()` pure function returning the Task-
   Tracker set. Links between drafts reference each other by UUID
   so the existing link-resolve pipeline picks them up
   automatically.
2. **`POST /api/projects/{slug}/sample-content` endpoint**
   (11b.1). Wraps the existing write paths, returns a per-
   collection / per-artifact summary, enforces "empty project"
   pre-flight with a typed `409`.
3. **Backend tests** (11b.1). Generator-shape + link-target
   resolution unit tests; integration tests for happy path +
   409-on-non-empty + 404-on-unknown-slug.
4. **Frontend: post-init choice screen** (11b.2). Extend
   `InitProjectDialog` with a stage enum
   `{ kind: "form" } | { kind: "done"; slug }`. The "done" stage
   renders three buttons: start-empty navigates; create-sample
   posts to the new endpoint then navigates; doorstop-import
   navigates + opens the existing doorstop dialog. API-layer
   additions: `sampleContent(slug)` client wrapper +
   `useCreateSampleContent(slug)` hook that invalidates caches.
5. **Frontend: empty-state button** (11b.2).
   `EmptyCollectionsState` gains a "Create sample content" action
   alongside its existing "New collection" guidance.
6. **Vitest** (11b.2). Three post-init flow tests + one
   empty-state button test.
7. **Coverage pass + Phase 11b "Shipped" note** (11b.3). Rerun
   `make test` + `make fmt-check` + `make lint`; confirm backend
   line coverage stays ≥ 80 %. Update this block with the shipped
   note per the Phase 7 / 8 / 9 / 10a / 10b / 10c / 11a template.

**Status (Phase 11b):** Shipped. Backend 692 tests (469 unit + 223
integration) / frontend 185 tests green after 11b.1 + 11b.2;
backend line coverage 85.15 % and function coverage 82.42 %, both
comfortably over the ≥ 80 % gate (lines +0.27 pp vs 11a's 84.88 %;
functions essentially flat at −0.02 pp). The "Create sample
content" choice from `UX-initSampleContent` is live on two
surfaces: the post-init choice screen in the init wizard, and the
empty-project fallback on the project detail page.

11b.1 shipped the backend:

- New `src/sample_content.rs`. `generate(slug)` is a pure-compute
  function returning three collections — REQ (3 requirements),
  DES (2 design docs), UC (2 use cases) — with seven artifacts
  total linked end-to-end via `satisfies`, `derives-from`, and
  `verifies`. The scenario is a "Task Tracker" app: familiar to
  the ReqForge target audience, legible in thirty seconds, and
  exercises the graph + matrix + coverage-matrix reports
  meaningfully. UUIDs are pre-allocated so draft-to-draft links
  resolve through the existing link-hint pipeline at load time.
  All artifacts are content-hosted — blob / URL samples need
  binary payloads and are deferred.
- `POST /api/projects/{slug}/sample-content` — returns `201` on
  success with a per-collection summary, `409` if the project
  already has any collection (refuses to overwrite real work —
  no `force` flag by design), `404` for an unknown slug. Writes
  go through the existing `write_artifact_file` +
  `atomic_write` + `reconcile_ownership` stack — no new write
  surface — then `state.refresh()` so discovery picks up the
  seeded files before the response returns.
- `SampleContentResponse` DTO carries per-collection summaries
  (`prefix`, `directoryName`, `artifactCount`, `artifactNames`)
  so the UI can confirm the seed with a compact summary and
  jump the operator to a specific artifact if desired.
- 9 generator unit tests (three collections, distinct prefixes,
  artifact count, UUID uniqueness, link-target resolution,
  hint-slug propagation, all three link types exercised, every
  artifact in the graph, safe-filename names) + 4 integration
  tests (happy-path 201 + files-on-disk, 409 on re-run, 404 for
  unknown slug, seeded-project listing shows the three new
  collections).

11b.2 shipped the frontend:

- `InitProjectDialog` is now two-stage. The form stage is
  unchanged; on init success, the dialog advances to a three-
  button choice stage (Start empty / Create sample content /
  Import from doorstop), matching `UX-postInitChoice` at a
  minimum. "Create sample content" POSTs the new endpoint and
  closes on success; a `409` surfaces inline without closing so
  the operator can pick a different path. "Start empty" and
  "Import from doorstop" both close + navigate, leaving the
  operator on the project page where the existing doorstop
  import dialog is one click away.
- `EmptyCollectionsState` gains a `projectSlug` prop and
  exposes a "Create sample content" button alongside its
  existing guidance, so operators who skipped the post-init
  choice can still seed the demo content from the project
  detail page.
- API layer: `SampleContentResponse` types,
  `api.createSampleContent` fetch wrapper, and a
  `useCreateSampleContent` mutation hook that invalidates all
  caches on success (the seed writes a project's worth of
  collections + artifacts — every cached listing is stale
  afterwards).
- 4 Vitest files covering the dialog flow (advances to the
  choice screen after init, "Create sample content" posts +
  closes on success, `409` error surfaces inline without
  closing, "Start empty" closes without any seed request) + 3
  covering `EmptyCollectionsState` (guidance + button render,
  button posts, `409` body surfaces in the inline error).

### Phase 11c — Onboarding polish

**Outcome.** Three concrete deliverables plus a bounded
accessibility pass — the last chunk of v1 polish before the
feature set is frozen:

1. **System config banner** (`UX-systemConfigBanner`). On the
   System Home view, when two or more projects are mounted but
   no System config has been loaded, show a banner inviting the
   operator to create one. ReqForge never writes the System
   config unbidden — the banner is guidance, not a wizard.
2. **Keyboard shortcut documentation + in-app help**
   (`UX-keyboardShortcuts`). A `docs/keyboard-shortcuts.md`
   reference + an in-app overlay triggered by the `?` key (and
   a header button) that lists the Markdown-editor shortcut
   set, the global `Ctrl/Cmd+S` save, and `Escape` for dialogs.
3. **Accessibility pass-through audit** (`UX-accessibility`).
   Mechanical fixes landing the "reasonable by construction"
   bar from INTENTIONS.md: a skip-to-main-content link,
   `aria-current="page"` on the sidebar NavLinks, focus-visible
   ring styles so keyboard nav is legible, one-pass review of
   every dialog for the three baseline attributes (`role`,
   `aria-modal`, `aria-labelledby`). Most are already there
   from prior phases — this commit surfaces the gaps.

The sub-phase split mirrors prior phases: **11c.1** backend
`GET /api/system` endpoint + tests; **11c.2** frontend surface
(banner + keyboard-shortcuts overlay + accessibility polish) +
Vitest; **11c.3** docs (keyboard-shortcuts + accessibility) +
shipped note.

**Locked decisions:**

- **Small backend surface.** One new endpoint:
  `GET /api/system` returning `{ loaded: bool, name?: string,
projectCount: number }`. Purely a view over existing state —
  no writes, no config mutation. Matches INTENTIONS.md's "never
  writes a System config unbidden".
- **Banner is dismissible-per-session.** Once shown, the
  operator can dismiss it (local state, not persisted) so the
  home page isn't noisy after an explicit "I know, I'm not
  using a System" decision. No backend state for the dismissal
  — fresh session re-shows it, matching the ephemeral posture
  of the other session-only data (LLM privacy acks, health
  tracking).
- **Keyboard help overlay via `?`.** Matches the universal
  convention (GitHub, Gmail, Notion). The `?` handler is global
  — attached in `AppShell`, scoped off when an input or
  contenteditable is focused. A header button offers a mouse-
  path equivalent.
- **Documentation, not feature.** The overlay documents what
  ReqForge already does — no new shortcuts are added. Per
  `UX-keyboardShortcuts`: CodeMirror defaults + `Ctrl/Cmd+S`
  save + `Escape` close-dialog is the whole set.
- **Accessibility audit is scoped.** Not a WCAG 2.1 AA
  certification — just the "reasonable by construction" bar
  from INTENTIONS.md. Deliverables: skip-to-main link,
  `aria-current` on NavLinks, `focus-visible` ring classes
  (Tailwind utility), and a review of existing dialogs for the
  three baseline attributes. Anything beyond — color-contrast
  refinement, screen-reader walk-through, live-region
  announcements — stays deferred.
- **Skip link uses Tailwind's visually-hidden pattern**
  (`sr-only focus:not-sr-only`). Appears in tab order as the
  first focusable element; targets the main content region via
  a stable id on `AppShell`'s `<main>`.
- **Docs lands in `docs/keyboard-shortcuts.md` and
  `docs/accessibility.md`** — short reference pages, cross-
  linked from README.md's existing "Getting Started" list.
  Mirrors the 10c pattern that put `docs/mcp.md` alongside
  README.
- **Tests.** Backend integration test for `/api/system` across
  three project-count / system-state permutations. Vitest for
  the banner's conditional render + dismiss, the keyboard
  overlay (opens on `?`, closes on Escape, skipped when typing
  in an input), and the skip-link's visibility-on-focus
  behaviour.
- **No new dependencies.** Tailwind already exposes
  `focus-visible:` + `sr-only`; keyboard handling is plain DOM.
- **No breaking UI changes.** Every change is additive; existing
  tests for dialog flows stay green.

**Tasks:**

1. **`GET /api/system` endpoint** (11c.1). New handler over
   existing `LoadedSystem` state; DTO field for `projectCount`
   computed from the mounts already snapshotted on `AppState`.
2. **Backend tests** (11c.1). Integration tests against three
   project-count / system-state permutations: unnamed + multi-
   project (banner case), named + multi-project (quiet),
   unnamed + single-project (quiet).
3. **`SystemConfigBanner` component** (11c.2). New component
   conditionally rendered on `SystemHomePage` when
   `projectCount >= 2 && !loaded`. Dismiss button stores the
   decision in `sessionStorage` keyed by `projectCount` so it
   re-shows if another project is mounted later in the session.
4. **`KeyboardShortcutsOverlay` component** (11c.2). Modal
   listing the three shortcut groups (editor, save, dialog).
   Header button + global `?` handler in `AppShell`; the
   handler skips when `document.activeElement` is a text input
   or a contenteditable.
5. **Accessibility polish** (11c.2). `AppShell` gains a skip-
   to-main link + a stable `id="main"` on the main region.
   Sidebar NavLinks get
   `aria-current={isActive ? "page" : undefined}`. A small
   Tailwind pass in `index.css` adds a keyboard-visible focus
   ring on buttons + links.
6. **Vitest** (11c.2). Banner renders / dismisses / stays
   hidden per state; overlay opens on `?` + closes on Escape +
   skipped when input is focused; skip link is visually-hidden
   until focused.
7. **Docs** (11c.3). New `docs/keyboard-shortcuts.md` and
   `docs/accessibility.md`. README.md picks up cross-references
   in the existing "Getting Started" section.
8. **Coverage pass + Phase 11c "Shipped" note** (11c.3). Rerun
   `make test` + `make fmt-check` + `make lint`; confirm backend
   line coverage stays ≥ 80 %. Update this block with the shipped
   note per the Phase 7 / 8 / 9 / 10a / 10b / 10c / 11a / 11b
   template.

**Status (Phase 11c):** Shipped. Backend 696 tests (473 unit + 223
integration) / frontend 199 tests green after 11c.1 + 11c.2;
backend line coverage 85.16 % and function coverage 82.45 %, both
comfortably over the ≥ 80 % gate (essentially flat vs 11b's
85.15 / 82.42 — the phase is small-scope polish rather than a
new module). v1 onboarding is now complete: the System Home view
surfaces a banner nudging operators toward a System config when
they have multiple projects but none is loaded, the keyboard
shortcut set is documented both in-app and in `docs/`, and the
bounded accessibility pass landed the "reasonable by
construction" bar from INTENTIONS.md.

11c.1 shipped the backend:

- New `GET /api/system` handler returning `SystemStateResponse
{ loaded, name?, projectCount }`. Pure read — never writes the
  System config, matching INTENTIONS.md's "never writes a System
  config unbidden". `name` is omitted from the wire when absent
  so the frontend relies on key-missing semantics rather than
  branching on `null`.
- 4 integration tests covering the three banner-relevant
  project-count / system-state permutations — unnamed +
  multi-project is the banner case, named + multi-project
  stays quiet, and unnamed-but-single stays quiet — plus the
  zero-project edge case.

11c.2 shipped the frontend:

- `SystemConfigBanner` per `UX-systemConfigBanner` — surfaces on
  the System Home view when `projectCount >= 2 && !loaded`.
  Dismissal is per-session via `sessionStorage` keyed by
  `projectCount`, so another mount later in the session re-
  surfaces the banner. Points operators at the
  `REQFORGE_SYSTEM_CONFIG` env var rather than offering a
  wizard.
- `KeyboardShortcutsOverlay` per `UX-keyboardShortcuts` —
  documents the shortcut set already in ReqForge in a modal
  triggered by the header `?` button and a global `?` hotkey.
  The hotkey is suppressed while a text input, textarea,
  `<select>`, or contenteditable is focused so the Markdown
  editor keeps its literal `?` keystrokes.
- Accessibility polish per `UX-accessibility` (scoped to
  "reasonable by construction"): a skip-to-main link as the
  first focusable element (Tailwind `sr-only focus:not-sr-only`
  pattern, targets a stable `id="main"` on the scrolling
  content region), a `:focus-visible` outline ring on buttons /
  links / `role="button"` / `<summary>` so keyboard navigation
  is legible without affecting mouse clicks, and an audit of
  the sidebar (React Router's `NavLink` already sets
  `aria-current="page"` when active — no change needed).
- 14 new Vitest files covering the banner, the overlay, and
  the AppShell wiring (skip-link target, header button mounts,
  `?` opens overlay globally, `?` suppressed when an input is
  focused, header button opens overlay).

11c.3 shipped the docs:

- New `docs/keyboard-shortcuts.md` — reference page listing
  the three shortcut groups (Everywhere / Markdown editor
  defaults / macOS vs Windows). Cross-linked from README.md.
- New `docs/accessibility.md` — describes what's in place
  (semantic HTML, dialog attributes, skip-to-main,
  focus-visible, `color-scheme: light dark`) and what's
  explicitly deferred (WCAG 2.1 AA certification, focus trap
  in modals, live-region announcements, high-contrast-mode
  support, `prefers-reduced-motion`). Cross-linked from
  README.md.
- README.md picks up a "Keyboard shortcuts and accessibility"
  subsection in "Getting Started" pointing at both new docs.

### Phase 12a — LLM-assisted post-import link suggestion

**Outcome.** After a fresh import (notably from doorstop), run
LLM analysis across the imported artifact set and propose typed
links the operator can accept or reject. Closes the gap exposed
by the initial ReqForge dogfooding: doorstop trees in the wild
often arrive with `links: []` everywhere, so the value of a
connected requirements graph never materialises without help.
Builds on the LLM provider chain shipped in Phase 10a and the
typed link authoring shipped in Phase 3 — pure additions, no
new substrate.

The sub-phase split mirrors prior LLM phases: **12a.1** backend
infrastructure (suggestions module, declined-sidecar persistence,
HTTP endpoints) + backend tests; **12a.2** frontend UI (Suggested
Links inbox with Pending and Rejected tabs, manual analyze button,
post-import auto-prompt) + Vitest; **12a.3** coverage pass.

**Locked decisions:**

- **Trigger surface.** Both: an auto-prompt banner after a
  successful import (doorstop, sample-content seeding) offering
  "Analyze now" / "Skip", and a manual "Analyze and suggest
  links" button in the Project detail header next to "Import
  from doorstop" / "New collection". The button is disabled with
  a tooltip pointer to `/llm` when no LLM provider is configured.
- **Scope per call.** Full-project context in a single LLM call.
  When the constructed prompt exceeds the active provider's
  token budget, fall back to per-Collection chunks and merge the
  responses. Phase 10a's adapter exposes the per-provider
  budget; feature-level code drives the chunking decision.
- **Suggestion shape.** `{ id, from, to, linkType, confidence,
rationale }` per proposal. `id` is a UUIDv7 minted at proposal
  time so accept / reject / reinstate URLs are stable. `confidence`
  is `0.0–1.0`. `rationale` is a short human-readable string the
  LLM produces. The link catalog (`TRACE-linkCatalog`) is
  unrestricted by default; an operator-defined per-Project
  whitelist is deferred.
- **Review surface.** A dedicated "Suggested links" tab on the
  Project detail page with two sub-tabs:
  - **Pending** — one row per active proposal with per-row
    accept/reject and a bulk "Accept all above N% confidence"
    action.
  - **Rejected** — one row per declined proposal, showing the
    original `{ from, to, linkType, confidence, rationale }`,
    the rejection timestamp, and a "Reinstate" action that
    promotes the suggestion back to actionable state (one click
    accepts; the proposal moves out of Rejected). The existing
    System-wide review queue stays focused on artifact reviews.
- **Persistence of declines.** A per-Project sidecar at
  `artifacts/.suggestions/declined.json`, written via the
  existing `write::atomic_write` path. Rejections persist across
  container restarts and are committable so the rejection
  history travels with the repo. Re-runs of the analysis filter
  out previously declined `(from, to, linkType)` triples by
  default. The Rejected tab gives the operator explicit access
  to revisit declined suggestions when new context emerges.
- **Pending persistence.** A sibling sidecar at
  `artifacts/.suggestions/pending.json`. Lets an analysis run
  produce a queue the operator works through over multiple
  sessions — proposals don't evaporate when the container
  restarts, and the inbox is the single source of truth for
  what's still actionable.
- **Failure mode.** Phase 10a fallback chain semantics. When the
  chain is exhausted the analysis run fails with a typed error;
  the import or manual run completes its primary task and
  reports the analysis failure separately so the operator can
  fix LLM config and rerun.
- **Privacy.** Reuses Phase 10a's one-time-per-container-
  lifetime privacy ack. No new surface.

**Tasks:**

1. **`suggestions` module** (12a.1). New
   `src/suggestions/` with `declined.rs` and `pending.rs`
   (sidecar JSON schemas, atomic read/write through
   `write::atomic_write`), `engine.rs` (LLM prompt construction
   and JSON-array response parsing), `errors.rs`. Public API:
   `propose_links(project, link_catalog) -> Result<Vec<Suggestion>, _>`,
   `record_decline / record_accept / reinstate_decline`.
2. **LLM prompt design** (12a.1). One feature-level prompt that
   ingests the Project's artifact set + the link catalog and
   returns a JSON array of suggestions. Reuses Phase 10a's
   prompt-abstraction adapter — no adapter-layer changes.
3. **Token-budget chunking fallback** (12a.1). When the
   full-project prompt exceeds the active provider's budget,
   chunk by Collection and merge results, deduplicating by
   `(from, to, linkType)`.
4. **HTTP endpoints** (12a.1):
   - `POST /api/projects/{slug}/suggestions/links/analyze` —
     run analysis, persist results to `pending.json`, return
     the new suggestion list.
   - `GET /api/projects/{slug}/suggestions/links` — pending list.
   - `GET /api/projects/{slug}/suggestions/links/declined` —
     declined list.
   - `POST /api/projects/{slug}/suggestions/links/{id}/accept` —
     create the link via the existing typed-link-authoring
     surface, drop the suggestion from pending.
   - `POST /api/projects/{slug}/suggestions/links/{id}/reject` —
     move the suggestion from pending to declined.
   - `POST /api/projects/{slug}/suggestions/links/{id}/reinstate` —
     accept a previously-declined suggestion (creates the
     link, drops the entry from declined).
5. **Backend tests** (12a.1). Engine tests against a test-only
   LLM stub; HTTP integration tests for each endpoint;
   pending / declined / reinstate roundtrip; chunking with a
   synthesized large project; Phase 10a fallback-chain
   exhaustion path.
6. **"Suggested links" tab** (12a.2). New tab on the
   Project detail page with Pending and Rejected sub-tabs. Per
   the locked-decision shape: per-row accept/reject + bulk
   accept-above-threshold on Pending; reinstate action on
   Rejected.
7. **Manual "Analyze and suggest links" button** (12a.2). New
   button in the Project header. Disabled with a tooltip
   pointing at `/llm` when no LLM provider is configured.
8. **Post-import auto-prompt** (12a.2). After a successful
   doorstop import / sample-content seed, surface a transient
   notice with "Analyze now" / "Skip" actions.
9. **Frontend Vitest** (12a.2). Tab render with happy path,
   accept/reject mutations, reinstate mutation, no-LLM-
   configured arm, post-import-auto-prompt arm.
10. **Coverage pass + Phase 12a "Shipped" note** (12a.3). Rerun
    `make test` + `make fmt-check` + `make lint`; backend line
    coverage stays ≥ 80 %.

**Deferred from this phase:**

- Cross-Project link suggestions (the System-wide graph).
- Code-traceability-driven suggestions — those flow from the
  Phase 9 scanner surface and form a distinct source of
  proposals that should land in its own work item.
- Operator-defined per-Project link-type whitelist.
- Automatic re-evaluation of declined suggestions when their
  endpoints are edited substantially. Reinstate is operator-
  driven for now.

**Status (Phase 12a):** Shipped on branch
`30-llm-recommendations-for-links-between-artifacts`. Eight
atomic commits split across the locked sub-phases:

- 12a.1 backend: sidecar persistence (`ec35038`), engine
  prompt + parser + propose_links (`b971fce`), HTTP endpoints
  for analyze / list / accept / reject / reinstate
  (`2250e80`), token-budget chunking fallback (`d9bbb99`).
- 12a.2 frontend: Suggested-links tab with Pending and
  Rejected sub-tabs + Manual analyze button + per-row
  Accept / Reject / Reinstate (`d0e73a8`), post-import
  auto-prompt in DoorstopImportDialog (`672d4b2`).
- Coverage closure: wiremock-driven analyze success-path test
  (`69709a7`).

Backend gains 29 unit tests across the suggestions module + 12
integration tests in `tests/suggestions_links.rs` (the latter
including the wiremock happy path and a malformed-output 502
arm). Frontend gains 6 Vitest cases on the new tab + 1 on the
DoorstopImportDialog auto-prompt addition. End-to-end pipeline
verified in dogfood: analyze → pending.json on disk → operator
accepts → real link appears on the from-artifact's
frontmatter.

11a-style "Shipped note" details are preserved in the
respective per-commit messages rather than re-inlined here.

### Phase 12b — LLM-assisted on-change link suggestion

**Outcome.** When an artifact is created or modified, analyze
the change in the context of its neighbors and surface link
suggestions or proposed changes to existing links (add a new
typed link, retype an existing one, flag a stale link for
removal). Complements 12a's batch one-shot with a steady-state
surface that keeps the link graph healthy as the requirements
set evolves. Same Phase 10a / Phase 3 substrate.

**Open questions to lock when planning starts:**

- **Trigger timing.** On save, on commit, or debounced while
  typing? Each trades token cost against latency and noise.
- **Context size.** Just the changed artifact plus direct
  neighbors, the whole containing Collection, or the full
  Project? Likely default "neighbors + Collection" with an
  option to widen.
- **Surface.** Inline panel on the artifact edit view,
  notification on the Project page, or a 12a-style queue?
  On-change semantics push toward inline.
- **Existing-link changes.** When the LLM proposes "this
  `derives-from` should be a `refines`," how is the retype
  authored without losing review history — same path as a
  manual retype, or a distinct accepted-suggestion event?
- **Auto-accept thresholds.** Never (always require operator
  click), opt-in for high-confidence proposals, or never
  contemplated at all? Probably never auto-applied: the
  review-workflow guarantees rest on every state change being
  human-reviewed.
- **Privacy ack.** Reuses Phase 10a's one-time-per-container-
  lifetime warning; no new surface needed.

**Deferred from this phase:**

- Transitive re-analysis when neighbors change. Initially only
  the directly-edited artifact triggers a suggestion run; if a
  neighbor changes, the panel for _this_ artifact does not
  re-run automatically.

**Status (Phase 12b):** Unstarted.

### Phase 13 — In-app LLM configuration

**Outcome.** Operators add, edit, delete, enable, and reorder
LLM providers from the `/llm` page. API keys go in form fields
that get persisted directly to the System config file — no
environment-variable indirection, no compose-file edits, no
container restart per change. Closes the dogfooding gripe that
configuring even one provider required edits in three places.

**Locked decisions:**

- **Secrets stay in `system.json`.** API keys are written into
  each provider entry's `apiKey` field. The System config lives
  in the operator's workspace, outside any tracked Project repo,
  so the git-tracking concern that originally drove env-var
  indirection doesn't apply. POSIX hosts get a file-mode check
  (loader rejects world-readable System config) so a stray
  permission can't leak the keys regardless.
- **`apiKeyEnvVar` is removed.** Schema bumps the System config
  to v2; the v1 → v2 migration step reads the named env var
  once at upgrade time, writes the value into `apiKey`, and
  drops `apiKeyEnvVar`. Operators upgrade through the existing
  Migrate-schema banner. The runtime keeps no env-var lookup
  path after the migration runs.
- **Per-provider `enabled` flag.** Each entry gains an
  `enabled: bool` (default true). The fallback chain skips
  disabled entries. "Select one provider to use" emerges by
  enabling exactly one; multi-provider fallback stays available
  by leaving several enabled.
- **CRUD endpoints take an index, not a UUID.** Provider
  ordering IS priority, so the URL is
  `/api/llm/providers/{index}` for `PUT` / `DELETE` /
  `PATCH(enabled)`; `POST /api/llm/providers` appends and
  optional `?after=index` lets the operator drop the new entry
  at a specific priority. This matches Phase 10a's existing
  index-based retest / acknowledge-privacy URLs.
- **Atomic writes through `write::atomic_write`.** CRUD writes
  rewrite the whole `system.json` so partial states can't
  appear on disk. The loader and writer share a typed
  `SystemConfig`, so operators and the UI converge on the same
  schema.
- **Read-only mounts surface a typed error.** When the System
  config file is mounted read-only (a common compose-file
  default), `POST` / `PUT` / `DELETE` return 409 with a body
  pointing at the docker-compose `:ro` flag. The operator fixes
  the mount and retries.

**Tasks:**

1. **System config schema v2** (13.1). Add an `apiKey:
Option<String>` and `enabled: Option<bool>` to
   `ProviderConfig`. Drop `apiKeyEnvVar`. Bump
   `CURRENT_SYSTEM_VERSION` to 2.
2. **Migration step v1 → v2** (13.1). New
   `schema_migration::system::v1_to_v2`: walks the `llm`
   array, for each entry reads `apiKeyEnvVar` from the env if
   set, copies into `apiKey`, drops the old field. Tests cover
   the env-set / env-missing arms.
3. **Adapter rewire** (13.1). `Adapter::api_key_available` and
   the `send_prompt` paths read `cfg.api_key` directly; the
   `read_api_key` helper goes away. Anthropic / OpenAI / Gemini
   adapters pick up the new field.
4. **File-mode check on POSIX** (13.1). `system::load` rejects
   System config files whose mode has the world-readable bit
   set. Returns a typed error naming the path and suggesting
   `chmod 0600`. Cargo cfg-gates the check to non-Windows.
5. **CRUD endpoints** (13.1). `POST /api/llm/providers`,
   `PUT /api/llm/providers/{index}`,
   `DELETE /api/llm/providers/{index}`,
   `PATCH /api/llm/providers/{index}` with an `{ enabled?:
bool, position?: usize }` body. Each rewrites
   `system.json` via `write::atomic_write`. 409 when the file
   is read-only.
6. **Backend integration tests** (13.1). Roundtrip per
   endpoint; migration roundtrip; file-mode rejection arm;
   read-only-mount 409.
7. **`/llm` page extension** (13.2). Add Provider form (new
   row, all fields editable). Per-row Edit / Delete / Enable
   toggle. Reorder via up/down buttons (drag-and-drop is
   deferred). Optimistic update through react-query.
8. **Frontend Vitest** (13.2). Form submit happy path, edit
   roundtrip, delete with confirmation, enable toggle, reorder.
9. **Coverage pass + Phase 13 "Shipped" note** (13.3).

**Deferred from this phase:**

- Drag-and-drop reordering (up/down buttons cover the case
  initially).
- Encrypted-at-rest secrets. The single-user-localhost posture
  plus mode 0600 covers the threat model on disk; encryption
  is deferred until a multi-user or remote-host deployment
  drives the need.
- Per-Project provider selection. Today every Project shares
  the System-level chain. A per-Project override lands when a
  use case actually requires it.

**Status (Phase 13):** Shipped on the same branch as Phase 12.
Ten commits, with substantial dogfood-driven follow-ups after
the initial five:

- 13.1 backend: schema v2 + ProviderConfig refactor (`apiKey`,
  `enabled`) + adapter rewire to read keys directly (`8f683f0`),
  POSIX file-mode 0600 enforcement at load (`2efea09`),
  CRUD endpoints with merge-on-PUT semantics for `apiKey` /
  `enabled` (`2493e08` + `63135f3`), per-request timeout override
  on `PromptRequest` with a 5-minute budget for analyze
  (`cf956b2`).
- 13.2 frontend: Add provider form + per-row Edit / Delete /
  Enable + reqwest-error chain surfacing (`4c2af6f` + `63135f3`),
  unconditional `/llm` sidebar link so first-time operators
  reach the page (`6859bbb`).
- Dogfood-driven fixes: reviewer text input renders directly
  when no presets exist (`4c16a03`), per-row mutation hooks so
  Accept / Reject in the Suggested-links tab don't disable
  siblings (`08e4d51`), doorstop YAML source synced for the
  three Phase 13 LLM requirements so re-imports stop clobbering
  the new content (`7446268`).

Backend tests include the schema-migration round-trip
(env-var → on-disk apiKey), the file-mode rejection arm (POSIX
only), eight CRUD integration tests covering POST / PUT /
DELETE / PATCH plus merge-on-PUT preserves-existing-key /
enabled semantics. Frontend tests cover the Add form happy
path, the openai-compatible-requires-endpoint client-side
validation arm, the Edit form prefill round-trip, the enabled
toggle, and the delete-with-confirm flow.

End-to-end verified in dogfood: configured a local Ollama
instance (`qwen2.5-coder:32b`) entirely through the UI, edited
the model name via Edit, ran the Phase 12a analyzer to
completion against 183 artifacts. The original gripe driving
the phase ("editing one config touches three places") is gone.

## Deferred Features

Every deferred feature tracked in `INTENTIONS.md` (Interop and
Deferred Features section) gets a Work Item only when a concrete need
triggers it — not on this roadmap. Deferred items include:

- Publishable HTML site for whole-System static publish
- Headless CLI / export for CI use
- PDF export
- Regulatory-formatted outputs
- LLM-assisted requirements extraction from monolithic documents
- LLM-assisted requirements extraction from code and tests
- Auto-grouping by code structure
- MCP read-write operations
- Multi-user authentication and authorization
- WebSocket bidirectional streaming
- Graph view beyond ~500 nodes
- Matrix view beyond ~500 items per axis
- WYSIWYG Markdown editing mode
- Headless CLI bulk schema-migrate

Each keeps its home in `INTENTIONS.md`'s deferred list; this roadmap
will gain Work Items for them only if and when they move into scope.

## Minor Considerations Parked

The "Minor Considerations Parked for Later" section of
`INTENTIONS.md` holds small issues (LLM token-window limits, UUID
collisions within a System, index-rebuild wall-clock at close-to-
target scaling, self-links) that we consciously didn't design around.
Each becomes a bug / feature request if it bites; none is on the
roadmap preemptively.
