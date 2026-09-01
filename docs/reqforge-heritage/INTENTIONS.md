# ReqForge Intentions

## Purpose

ReqForge is a **traceability-first** requirements and artifact management
system for engineering and product teams. It manages the full lifecycle
of project artifacts — requirements, design documents, use cases,
diagrams, roadmaps, uploaded documents, and references to external
content — and the typed links between them.

ReqForge is inspired by [doorstop](https://github.com/doorstop-dev/doorstop)
but aims to be significantly faster, comprehensive in artifact scope,
and built around a richer traceability model.

## Scope and Audience

ReqForge targets **personal projects and small-to-medium engineering
teams** doing requirements, design, and traceability work without the
burden of formal accreditation, regulator sign-off, or
compliance-driven audit trails. Teams with those needs are better
served by commercial tools (Jama, Polarion, IBM DOORS) that invest
heavily in the regulator-facing side. ReqForge may evolve toward
regulated use over time, but that is explicitly not the initial
target — regulatory features are deferred until a concrete need
drives them, and users who require them today should pick a tool
aimed at that audience.

The initial version assumes **a single user** accessing the web UI
from the host that runs the container (or native dev build). No
authentication is required and no authorization distinctions are
enforced between hypothetical concurrent users. Multi-user
authentication and authorization are explicitly deferred.

## Guiding Principles

- **Traceability is the point.** The link graph between artifacts is the
  primary first-class concept. Links are typed (derives-from, satisfies,
  verifies, conflicts-with, supersedes, …) and carry a stable UUID
  target plus a best-effort human hint.
- **Git is the storage layer, but ReqForge never touches git.** Artifacts
  live as files in the user's git repositories. ReqForge only reads and
  writes those files; the user performs commits, branches, merges, and
  conflict resolution through their normal git client.
- **File-per-artifact.** Each artifact lives in its own file, keeping
  diffs small and reviews meaningful.
- **Three artifact shapes, one graph.** Content-hosted, uploaded blob,
  and URL reference are three shapes of artifact; all participate
  uniformly in the traceability graph.
- **Stable UUID identity.** Every artifact carries a UUID assigned at
  creation. Filenames, prefixes, and titles can change; the UUID is
  what links reference.
- **Review is a log, not a flag.** Each artifact maintains a review
  history: who, when, outcome, and explanation. Rejected-with-TODOs
  blocks re-approval until those TODOs clear. Initial support is
  single-approver; multi-reviewer / N-of-M is planned for later.
- **Comprehensive, fast, and modern.** ReqForge should cover the
  artifacts engineering teams actually produce, run fast enough to stay
  out of the way, and offer a UI that's comfortable for extended use.
- **Schema versioning from day one.** Projects and artifacts carry
  schema versions so the on-disk format can evolve without breaking
  existing projects.

## Hierarchy and Terminology

ReqForge organizes work as **System > Project > Collection > Artifact**.

- A **System** is a named collection of Projects whose artifacts can
  link to one another — useful when a feature spans multiple
  repositories (related libraries, distributed-system services, or
  components of a larger system). A System is described by a System
  configuration file naming it and listing its expected Project slugs.
- A **Project** is one git repository. It is identified by a
  `reqforge.json` marker at its root declaring a stable project slug,
  independent of where the repository happens to be mounted.
- A **Collection** is a named, typed grouping of related Artifacts
  within a Project — for example, a requirements Collection, a
  design-documents Collection, a use-cases Collection. Each Collection
  has a prefix used in artifact identifiers and in link hints.
- An **Artifact** is an individual managed item within a Collection.

> Terminology note: earlier drafts used "Document" for the grouping
> (following doorstop). "Document" is too easily confused with
> document-shaped _artifacts_ (a design document, a standards PDF), so
> ReqForge uses "Collection" for the grouping and reserves "document"
> for its everyday meaning.

## Managed Artifacts

- Requirements
- Design documents
- Use cases
- Diagrams
- Roadmaps
- Uploaded documents (Microsoft Office formats, PDFs, images)
- URL references (external content viewable in a browser)
- Citations — bibliographic references to works that may or may
  not have a digital copy in ReqForge's reach; pointed at by
  `cites` links and carried as URL-reference artifacts,
  uploaded-blob artifacts, or content-hosted artifacts whose
  body is the formatted citation string
- Verifications — ordinary artifacts (typically prefix `VER`)
  that record a verification activity, whether automated test,
  manual procedure, observation, or document-review evidence
- Constraints — ordinary artifacts (typically prefix `CON`)
  that express limits on the solution space
- Additional artifact types as needs emerge

## Deployment Model

ReqForge runs as a Docker container with one or more git repositories
bind-mounted into it. Each bind-mounted repo is a Project; together,
the mounted repos form a System. Docker Compose is the recommended
orchestration mechanism for anything beyond a single mount.

**Mount convention.** Each repository is mounted at the top level (so
`.git` is visible) under a well-known in-container prefix (default:
`/repos`). ReqForge scans that prefix on startup for first-level
subdirectories containing `.git` and classifies each:

- `.git` and `reqforge.json` present → loaded as a Project.
- `.git` present, `reqforge.json` missing → surfaced as "not yet a
  ReqForge project" with an **Initialize as ReqForge project** action
  in the UI.
- `.git` missing → warning banner, mount ignored.
- Read-only mount → loaded read-only with a banner in the UI; writes
  disabled for that Project.

**Identity and collisions.** Project identity is the slug declared in
`reqforge.json`, not the mount path. If two mounted Projects declare
the same slug, ReqForge surfaces the collision as an error and
declines to operate on either until resolved.

**System integrity.** The System config file lists expected Project
slugs; missing mounts are surfaced ("expected project X not mounted")
so cross-Project links can still be interpreted meaningfully.

**Because ReqForge performs no git write operations:**

- The user manages clones, branches, commits, pulls, and pushes
  externally with their own git client.
- ReqForge detects external changes via filesystem polling (needed
  because inotify is unreliable across bind mounts on some hosts).
- Files written by ReqForge have their UID/GID adjusted to match the
  owner of each repository's `.git` entry, so edits don't appear as
  root-owned on the host.
- All ReqForge file writes are **atomic** (temp-file-then-rename
  with fsync), because the target audience runs ReqForge on
  home-lab and dev-container infrastructure where flaky restarts
  and power loss are routine.
- ReqForge binds its web UI to **`0.0.0.0`** so the container's
  published port reaches the host and beyond. Access control is
  the operator's responsibility via container networking or host
  firewall rules.

ReqForge uses a pure-Rust git library (**gitoxide** or equivalent)
for **read-only** access to the git object store where features
need historical context — specifically diff-against-prior-commits
and "since last review" comparisons. ReqForge does not expose
general git-client UX (branch lists, commit graphs, merge-conflict
resolution) — that remains the user's git client's job. ReqForge
never shells out to the `git` CLI.

**Configuration via environment variables.** `REQFORGE_MOUNT_PREFIX`
overrides the default `/repos` mount-scan root. `REQFORGE_SYSTEM_CONFIG`
points at the System configuration file when one is bind-mounted.
`REQFORGE_PORT` overrides the default port (**36743** — chosen to
dodge common developer-tool ports and because it spells "FORGE" on
a phone keypad). `REQFORGE_LOG_LEVEL`, `REQFORGE_UID`, and
`REQFORGE_GID` are available for log verbosity and manual ownership
overrides.

**Operator workspace convention.** A production operator maintains
a workspace directory separate from every managed repository,
conventionally **`~/.reqforge-workspace/`**, holding the
`docker-compose.yml`, `system.json`, and a `.env` file with secrets
(LLM API keys, etc.). Managed Project repositories live wherever
the operator normally keeps them and are bind-mounted into the
container independently.

**Developer workspace convention.** The ReqForge source repository
contains a **`.reqforge-workspace/`** directory at its root for
developer-time configuration and test fixtures, gitignored except
for committed example/template files. A contributor in the VS
Code devcontainer copies the examples, optionally places test
Project repositories under `.reqforge-workspace/test-repos/`, and
runs ReqForge natively (`cargo run`, `npm run dev`) with
`REQFORGE_MOUNT_PREFIX` pointing at the local test-repos
directory. The hidden name mirrors the operator convention so the
semantics are consistent between dev and production.

**Makefile targets.** Routine developer and operator operations
are exposed as `make` targets (`make dev`, `make build`,
`make test`, `make docker-build`, `make docker-run`,
`make docker-publish`, `make fmt`, `make lint`, and similar), so
nobody needs to memorise long command invocations. README recipes
reference the `make` targets rather than the underlying commands.
CI pipelines (GitHub Actions, GitLab CI, or similar) are
expected to drive these same `make` targets; the specific CI
platform is an implementer's choice and not a ReqForge-side
commitment.

**Observability.** A minimal surface — `/healthz` for liveness
probes, `/readyz` for readiness (returning 200 once project
discovery and indexes are built), optional `/metrics` in
Prometheus text format (opt-in via env var), and JSON logs to
stdout — is enough for single-operator deployment. No
distributed tracing or OpenTelemetry in v1.

**Container upgrade.** `docker compose pull` then
`docker compose up -d` from the operator workspace. On restart,
ReqForge rebuilds in-memory indexes and handles any file at an
older schema version via the Schema Evolution path (lazy
write-back). Files above the current schema trigger a
read-only-with-banner state for their Project. No dedicated
migration script is required.

## Onboarding

The first-run flow is designed to get a user from `docker compose up`
to a working artifact with minimal friction:

1. **Startup home view** lists every mounted repository annotated
   with its validity state (Project, Needs-init, No-git, Read-only).
2. **Project initialisation wizard** for any Needs-init mount —
   slug, name, optional description — writes `reqforge.json` and
   creates the empty Collections root.
3. **Post-initialisation choice** offers three ways to populate the
   new Project: **Create first artifact**, **Create sample
   content**, or **Import from doorstop** (the last only when
   `.doorstop.yml` markers are present in the mount).
4. **System configuration banner** appears on the home view when
   multiple Projects are mounted but no System config has been
   loaded, inviting the user to create one. ReqForge never writes
   a System config unbidden.

## User Experience

- **CRUD on artifacts and links** from the UI, with no direct file
  editing required.
- **Create-artifact UX split by shape:** an in-browser editor for
  content-hosted artifacts, an upload dialog for binary/complex
  formats (Office, PDF, images), and a URL-entry form for URL
  references.
- **Markdown authoring** is a side-by-side view: a CodeMirror-based
  text editor on one side, a live-rendered Markdown pane on the
  other. The text pane is the single source of truth; the rendered
  pane updates from it in real time. Text fidelity (indentation,
  comments, soft line breaks, list style) is preserved. A
  bidirectional WYSIWYG mode is deferred as an opt-in addition,
  since its round-trip through a shared AST would normalise
  hand-written text.
- **Upload is replace-only** for binary artifacts — updating them means
  uploading a new version. UUID, review log, and links survive the
  replacement.
- **Upload preview is tiered by format:** browser-native formats
  (PDF, common images, SVG, plain text) embed inline; Office and
  other complex binaries show a server-side thumbnail when one is
  available, or a generic icon otherwise; unknown formats show a
  file icon with a download link. Thumbnail generation for complex
  formats is a deployment-configurable capability, not a hard
  requirement.
- **Move and rename** artifacts freely across Collections; UUID is
  stable, so links are never broken and human-readable hints refresh
  lazily.
- **Link creation** is offered via three complementary affordances: a
  type-ahead picker while editing an artifact, a graph canvas with
  drag-to-link for visual authoring and exploration, and a matrix view
  for coverage and gap analysis.
- **Diff view** is shape-aware: textual diff for content-hosted
  artifacts, metadata + side-by-side rendered preview for binary
  blobs, and URL-string diff plus an external-content warning for URL
  references. No git commands are invoked; the view reads the working
  tree and accessible prior versions.
- **Search** is full-text across the System, filterable by type,
  review state, link presence, and Project.
- **Browsable title-indexed views** per artifact type (Design
  Documents, Standards, Use Cases, Diagrams, etc.) provide a scannable
  overview for artifacts whose titles carry meaning.
- **Review UI:** per-artifact review pane showing the review log,
  unresolved blocking TODOs, and a "Since last approval" section
  with both a content diff and a review activity timeline (every
  rejection/TODO event between the last approval and now, with
  resolved vs. unresolved TODO state visibly distinguished).
  Actions include approve, reject-with-TODOs, add/resolve TODO,
  and re-request review. A System-wide review queue has two
  sections: artifacts awaiting review (default sort:
  oldest-modification-first) and artifacts waiting on unresolved
  blocking TODOs from the author.
- **Large lists** — Collection artifact lists, search results,
  review queue, and report tables — use virtualised rendering
  (continuous scroll, no pagination UI) so thousands of entries
  stay responsive.
- **Browser support** targets the current stable release of
  Firefox, Chrome, Safari, and Edge. IE is not supported; mobile
  browsers are not an optimisation target. The docs make this
  expectation prominent.
- **Keyboard shortcuts** are limited to the standard Markdown
  editor set (CodeMirror defaults plus `Ctrl/Cmd+S` save).
  ReqForge-specific navigation shortcuts are deferred until a
  concrete request specifies them.
- **Accessibility** is treated as "reasonable by construction"
  — semantic HTML, keyboard-operable controls, ARIA where
  native semantics fall short — without a formal WCAG 2.1 AA
  commitment.
- **Undo/redo** works in-session for the editor, link
  create/delete, and artifact create / delete / move / rename.
  Cross-session undo is git's job.

**Performance and scaling targets** (soft, not contractual):
designed for a home-lab / small-team workload — roughly 10
mounted Projects and 5,000 total artifacts per System. Within
that envelope, UI page transitions aim for < 200 ms P95, API
read fetches < 50 ms P95, full-text search across 10k artifacts
< 500 ms P95, cold UI load < 3 s, and editor keystroke-to-
preview < 100 ms. Beyond the envelope ReqForge still works but
performance is best-effort; tuning is revisited when real
workloads exceed the target.

## Link Types

ReqForge ships with a built-in catalog of seven link types covering
the core relationships engineering teams express:

| Forward          | Inverse         | Directed | Acyclic | Meaning                                                             |
| ---------------- | --------------- | -------- | ------- | ------------------------------------------------------------------- |
| `derives-from`   | `derived-into`  | yes      | yes     | Source is a refinement of the target (child → parent).              |
| `satisfies`      | `satisfied-by`  | yes      | no      | Source fulfils the target.                                          |
| `verifies`       | `verified-by`   | yes      | no      | Source is evidence the target holds (typically test → req).         |
| `supersedes`     | `superseded-by` | yes      | yes     | Source replaces the target.                                         |
| `cites`          | `cited-by`      | yes      | no      | Source references the target as an external or historical citation. |
| `conflicts-with` | _self-inverse_  | no       | no      | Unresolved conflict between the pair.                               |
| `related-to`     | _self-inverse_  | no       | no      | Weak, untyped association — escape hatch.                           |

**Storage is one-sided.** A link is stored in the source artifact's
metadata only; the reverse view is derived from the UUID index. One
source of truth, half the writes, no risk of divergent records.

**Pairings are permissive.** Any link type may relate any two
artifacts regardless of Collection or artifact type, aligned with
doorstop's practical posture.

**The catalog is extensible.** The System configuration file may
declare additional link types beyond the built-in six. Each declared
type carries the same metadata shape (forward name, inverse name,
directedness, acyclicity). Built-in types are always available and
cannot be overridden.

## Code and Test Traceability

ReqForge supports typed traceability from source code and test
files to requirements via a **scan-and-report** mechanism. The
scanner produces overlay data on demand rather than persisting
code references as first-class artifacts — the link graph stays
clean; code traces feed reports and coverage calculations.

**Language registry.** The scanner uses a registry of supported
languages, each declaring file-extension globs and comment syntax.
Built-ins cover Rust, Python (with triple-quoted docstrings
treated as comments for tag purposes), JavaScript and TypeScript,
POSIX shell, and Dockerfiles. The System configuration file may
declare additional languages. YAML is deliberately out of scope.

**Tag format.** Requirement tags appear only inside comments and
take the form `<Verb>: <id>[, <id>]...`, where `<Verb>` is one of
the built-in link types (`Satisfies:`, `Verifies:`,
`Derives-From:`, etc.). `Implements:` and `Requirements:` are
accepted aliases for `Satisfies:`. Multiple IDs per tag are
allowed; a trailing comma continues the list onto following
comment-only lines.

**Human-readable IDs, not UUIDs.** Source tags reference artifacts
by `(collection-prefix, artifact-name)` rather than UUID. Source
comments stay legible; the trade-off is fragility under rename,
which the orphan-tag report surfaces when it occurs.

**Scan configuration.** Each Project's `reqforge.json` may declare
source paths to scan; sensible defaults (like `src/` and `tests/`)
apply otherwise. Common ignore directories (`.git`,
`node_modules`, `target`, `__pycache__`, `.venv`, etc.) are
excluded from the walk.

**Coverage expectation.** Each Collection carries an
`expectsCodeTrace` flag (default `true`); individual artifacts may
override it. Reports exempt no-trace-expected artifacts from
"uncovered" counts, generalising doorstop's non-functional
exemption.

Implementation of the scanner is deferred and will be written in
Rust inside ReqForge's back-end. The existing
`scripts/traceability.py` is a **design reference**, not code to be
preserved, vendored, or shelled out to from ReqForge.

## Traceability Reports and Exports

ReqForge produces traceability reports as a first-class capability. All
reports are viewable in the UI, take a scope selector (whole System /
single Project / single Collection / user filter), and can be exported
as downloadable files.

**Report classes shipped initially:**

- **Coverage matrix** — for each parent artifact, the child artifacts
  that link to it; parents with no coverage are flagged as gaps.
  The default covering-link-type set is `satisfies` + `verifies`
  (user-configurable per report).
- **Impact analysis** — given an artifact, everything transitively
  dependent on it (answers "what else needs review if I change this?").
- **Orphans** — artifacts with no incoming or outgoing links.
- **Conflicts** — pairs related by `conflicts-with`.
- **Cycles** — cycles in link types that should be acyclic.
- **Review status** — approved / rejected / unreviewed counts faceted
  by Project, Collection, and type; artifacts with unresolved blocking
  TODOs surface as a distinct status.
- **Unresolved links** — consolidated broken-link list across the
  System, each entry naming the project that must be mounted to
  resolve it.

**Baseline export formats:** HTML (hyperlinked, downloadable), CSV
(primarily for matrix-shaped reports), and JSON (programmatic
consumers).

**Inactive artifacts** are excluded by default from uncovered counts
in coverage and code-trace reports and from the System-wide review
queue. A "show all / include inactive" toggle reveals them. Tags
are usable as a grouping or filter axis on any report.

## Storage Rules

- In-tree storage for text artifacts and reasonably sized binary
  uploads.
- Large content too big for the git tree is represented as a
  URL-reference artifact instead of being stored in-tree.
- Git LFS is explicitly out of scope for the initial version. It may
  be revisited later if artifact sizes demand it.

## On-Disk Format

ReqForge uses **JSON** as its sole structured-data format — for
artifact metadata, all configuration files, and frontmatter blocks.
YAML and TOML are deliberately not used. JSON is chosen for parse
performance (roughly 3–4× faster than YAML in typical Rust parsers),
unambiguous syntax, and uniform tooling across the stack.

Content-hosted artifacts are **Markdown files with JSON frontmatter**,
one `.md` file per artifact. The Markdown body is the artifact's
prose; the frontmatter is its metadata. JSON frontmatter is
delimited with YAML-style `---` markers — since any valid JSON is
also valid YAML flow-style, GitHub, GitLab, Pandoc, Jekyll, Hugo
and similar renderers display the frontmatter block as a YAML
table without needing a ReqForge-specific extension.

Uploaded-blob artifacts are **a binary file paired with a
`.reqforge.json` sidecar** (e.g., `DES-spec.pdf` alongside
`DES-spec.pdf.reqforge.json`). URL-reference artifacts are **a single
`.reqforge.json` file** holding the URL and metadata.

Pure-metadata files use the compound extension `.reqforge.json`
(rather than plain `.json`) so ReqForge files are visually
identifiable in a repository tree and easy to select with globs.
Configuration files keep their specific filenames — `reqforge.json`
(project), `.collection.json` (per-Collection), and the System
configuration file — because those are recognised by name.

Typed links live **inline** within the owning artifact's metadata,
not in separate files. Each link carries the target UUID, link type,
and best-effort human hint.

By default, a Project's Collections live as subdirectories under an
**`artifacts/`** directory at the repository root. A Project may
override this root by declaring an optional `artifactsPath` field
in its `reqforge.json`; the override is expected to be rare. The
directory is called `artifacts/` rather than `requirements/` because
ReqForge manages more than just requirements, and rather than
`reqforge/` so the tool isn't branding a slot in the user's repo.

The detailed field-by-field schemas for project config, collection
config, System config, artifact metadata, links, and review log
entries are captured in the FORMAT requirements.

All ReqForge-authored files are **UTF-8** without a BOM, use **LF**
line endings on write (CRLF is tolerated on read), and JSON is
written pretty-printed with two-space indentation for git-friendly
diffs.

## Schema Evolution

ReqForge's on-disk formats will change over time. The evolution story
is designed to be safe, explicit, and minimally disruptive.

**Per-file-type versions.** Artifact metadata, project
configuration, per-Collection configuration, and the System
configuration each carry their own monotonic `schemaVersion`
integer. Schemas evolve independently; a change to one doesn't
force bumps in the others.

**Forward migration on load.** When ReqForge reads a file whose
`schemaVersion` is below the current known version, it applies a
registered migration function per single-step bump (`v1 → v2`,
`v2 → v3`, …) in sequence, producing the current in-memory
representation. Migration functions are deterministic: re-reading
a file many times produces identical in-memory output.

**Lazy write-back.** ReqForge does **not** rewrite files just
because they migrated on load. The on-disk `schemaVersion` bumps
only when the user actually edits the file — that edit would
rewrite the file anyway. A bad migration never touches
read-only files; the original bytes are always recoverable via
git.

**Bulk migrate in the UI.** A per-Project "Migrate this Project
to the latest schema" action rewrites every ReqForge-authored
file at once. Before running, ReqForge checks for an
uncommitted-changes tree and warns so the migration can be its
own commit; ReqForge itself never commits.

**Newer-than-current files are refused.** A file whose
`schemaVersion` is higher than the running ReqForge knows loads
the containing Project read-only with a banner prompting a
ReqForge upgrade. No guessing, no partial writes.

**Versioning policy.** Before 1.0, schema bumps are liberal;
after 1.0, schema bumps are breaking and ride with a ReqForge
major-version bump. Downgrading schemas is explicitly out of
scope — teams collaborating on a System run the same ReqForge
version.

A headless CLI bulk-migrate is deferred, riding with the broader
CLI/headless deferral.

## Technology Direction

- **Back-end:** Rust — for performance, reliability, and strong
  file-I/O and concurrency primitives.
- **Front-end:** React — for UI ergonomics and ecosystem breadth.

## LLM Integration

ReqForge's LLM integration is **optional**. When no LLM is
configured, all LLM-dependent UI affordances are hidden and core
operations proceed without LLM involvement. When one or more LLMs
are configured, they unlock features such as assisted rename
(Phase 10b) and link-suggestion analysis across an artifact set
(Phase 12a, post-import + on-demand), plus deferred future
features like document-to-requirements extraction, code-scan
extraction, on-change link suggestion (Phase 12b), and
auto-grouping by code structure.

**Providers.** Three adapter families ship initially: OpenAI-
compatible (covers OpenAI, Azure OpenAI, Ollama, LMStudio, vLLM,
OpenRouter, LiteLLM, and any other service with an OpenAI-
compatible base URL), native Anthropic, and native Google Gemini.

**Configuration.** The System config carries an `llm` array of
provider entries in priority order, each declaring provider,
model, endpoint (where relevant), an optional `apiKey` for
providers that need one, and an `enabled` flag. Configuration is
edited in the UI on the `/llm` page — Add / Edit / Delete /
Enable / reorder controls write the System config back to disk.
The fallback chain walks the array in priority order, skipping
disabled entries, so "select one provider to use" emerges as
"enable exactly one"; fallback across multiple healthy providers
remains the default for operators who want it. The System
config file lives in the operator's local workspace (outside
any tracked Project repo); on POSIX hosts the loader rejects
files whose mode is world-readable so a stray secret can't leak
through filesystem permissions alone.

**Fallback chain.** LLM-dependent features walk the provider
array in order; the first healthy provider wins. On failure,
ReqForge falls through to the next. When the chain is exhausted,
features with plain equivalents (e.g. rename) fall back to the
plain action; features without plain equivalents report the
failure.

**Health tracking.** Providers are tracked per-process in three
states: healthy, transient-degraded (exponential-backoff skip),
and hard-disabled (authentication/endpoint/model errors; removed
from rotation until the user triggers a "Re-test providers"
action after fixing config).

**Privacy.** Before artifact content is first sent to a cloud
provider within a container lifetime, a one-time warning names
the provider. Localhost and RFC 1918 private-IP endpoints skip
the warning.

**Extensibility.** The adapter is a generic prompt-response
surface rather than a rename-specific one, so new LLM-dependent
features slot in without touching the adapter layer.

**MCP server for AI coding agents.** In addition to ReqForge's
own inward-facing LLM features (rename suggestion, and the
deferred extraction/grouping items), ReqForge exposes a
**Model Context Protocol server** so AI coding agents —
Claude Code, Cursor, Zed, GitHub Copilot, and similar — can
query its artifacts, traceability graph, reports, and review
state as first-class operations rather than parsing raw files.
The MCP server is a thin adapter over the REST API, exposes
tools (artifact reads, search, graph walks, report generation,
review-log reads), resources (one per artifact, addressable by
stable URI), and canned workflow prompts (gap analysis,
coverage summary, review assist, implementation planning,
test-gap planning, impact-analysis narratives). It is
localhost-only and read-only initially; AI-driven writes are
deferred until the read-only surface is proven in practice,
at which point they land in the existing review workflow like
any other drafted artifact. The MCP surface is a first-class
feature, not a deferred future item — it substantially
magnifies the value of ReqForge's requirements to
agent-assisted coding workflows.

Implementation of LLM integration is deferred; the design above
locks in the shape so the feature can be built incrementally.

## Interop and Deferred Features

- **One-way import from doorstop** for bootstrapping existing
  projects. The importer maps doorstop documents to ReqForge
  Collections, translates each item's fields (including `active`,
  `derived`, `level` → `outlineLevel`, `ref`, reviewed hash, and
  untranslatable extension fields via `legacy`), and rewrites
  parent-child links as `derives-from`. Original doorstop files
  are left on disk untouched. Round-tripping to doorstop is
  explicitly out of scope.
- **Code and test traceability** scanner is deferred. The design
  lives in the "Code and Test Traceability" section above and will
  be implemented in Rust inside ReqForge's back-end.
  `scripts/traceability.py` is a design reference only, not code
  ReqForge will vendor or carry forward.
- **Publishable HTML site** (doorstop-style static publish of a whole
  System) is deferred; the baseline HTML export of individual reports
  covers the common case initially.
- **CLI / headless export** for CI use (for example, regenerating a
  published traceability site on every main-branch merge) is deferred.
- **PDF export** is deferred until concrete demand emerges; HTML/CSV/
  JSON cover the baseline.
- **Regulatory-formatted outputs** (signed PDFs, compliance templates,
  tamper-evident audit packages) are deferred until a concrete
  compliance requirement drives the work.
- **LLM-assisted requirements extraction from monolithic
  documents** — legacy specs, PDFs, long design notes. Proposes
  candidates; user reviews accept/modify/discard. Deferred.
- **LLM-assisted requirements extraction from code and tests** in
  repositories with no existing requirements. Proposes a first
  draft. Deferred.
- **Auto-grouping by code structure** — detect Rust workspaces,
  npm monorepos, Python packages, Docker Compose services, and
  similar across every language in the scanner registry, and
  propose Collection groupings. Deferred.
- **Multi-user authentication and authorization** — the initial
  single-user-localhost posture is sufficient for the target
  audience. A full authentication/authorization story (login,
  sessions, roles, permissions) is deferred until a concrete
  multi-user need emerges.
- **WebSocket bidirectional streaming** — Server-Sent Events is
  sufficient for the current change-notification needs.
  WebSocket is deferred until a feature genuinely requires
  bidirectional streaming.
- **Graph view scalability beyond ~500 nodes** — at that scale
  the UI prompts for filtering. Hierarchical overviews,
  on-demand node expansion, or dependency/status-based subgraph
  views are deferred until use cases exceed the current cap.
- **Matrix view scalability beyond ~500 items per axis** —
  similar story. Chunked views, focus-on-one-artifact fan-out
  views, or additional filter dimensions are deferred until
  they are needed.

## Inspiration and Differentiation

Doorstop established that requirements can live in a git repo as plain
files. ReqForge keeps that model and differs in:

- Treating typed traceability as the core feature rather than an
  afterthought, with an extensible set of link types.
- Separating stable UUID identity from human-readable naming, so
  renames and moves don't break links.
- Providing a richer review workflow with logs and blocking TODOs,
  plus dedicated review UI.
- Operating across multiple projects simultaneously via the System
  concept, with explicit System-level configuration.
- Explicitly staying out of git operations — the user's git client
  remains the git client.
- Being fast enough to stay out of the user's way.
- Offering a modern, polished UI with a graph canvas, matrix view,
  type-ahead link picker, and shape-aware diff and review tooling.
- Covering a much broader artifact scope, including uploaded
  documents and URL references.

## Minor Considerations Parked for Later

These are small items that are probably fine as designed but worth
revisiting if they become significant in practice:

- **LLM token-window limits.** If an artifact's content exceeds a
  provider's context window, the LLM call may fail or truncate
  silently. Graceful handling (explicit truncation, chunking with
  summarisation, or a clear error message) is not defined and can
  be specified when the case arises.
- **UUID collisions within a System.** UUIDv7 makes this
  vanishingly unlikely, and no explicit collision-handling path is
  specified. Revisit if a collision ever occurs in the wild.
- **Index rebuild wall-clock time at close-to-target scaling.**
  Performance targets cover the cold UI load but not the back-end
  index rebuild specifically. Benchmark when the implementation
  reaches close-to-target workloads.
- **Self-links (A → A).** Permissible under permissive pairings
  (per `TRACE-permissivePairings`) but semantically unusual; no
  explicit UI treatment is specified. Revisit if it ever turns up
  in practice.

## Open Questions

- Implementation roadmap — build order (storage layer → HTTP API →
  React shell → editor → scanner, etc.).
