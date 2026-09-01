# Installing & Running provreq Against a Subject

How an operator gets `provreq` running against a real subject repository — on Linux,
macOS, or Windows, and inside a Docker / dev container. The motivating case is
**qrusty**, which has its own dev container and build that we do not run from here.

This is the **consumer** side. It complements [build-and-release.md](build-and-release.md)
(the **producer** side — how the binaries are built and published) and
[applying-to-existing-repos.md](applying-to-existing-repos.md) (the **architecture** —
where artifacts live and how the tool is deployed). Read those for the _why_; this doc is
the _how_.

Status: **guidance settled.** The three decisions below were taken in the 2026-09-01 roadmap
discussion; anything marked _future work_ is deliberately not scheduled.

## The one principle everything follows from

**provreq runs where the subject builds.** Per A5-Option-B, the brain and the executor ship
together and are installed _into the subject's own dev environment_ — "the
ProvableRequirements container never learns to build the subject." So "use provreq in qrusty"
means: get the `provreq` binary, plus the engines qrusty's requirement categories need, into
qrusty's dev container, and point provreq at the checked-out tree. Everything below is a
variation on _how you get those two things into a given environment_.

## The split that decides whether the OS matters: brain vs. executor

The useful first question is not "which OS" — it is "what are you doing", because that
decides whether an engine (and therefore the subject's toolchain, and therefore the OS) is
involved at all.

| Task                                                               | Commands                                                   | Needs                                                                            | Portable?                              |
| ------------------------------------------------------------------ | ---------------------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------- |
| **Brain** — browse, triage, draft, read back, report, serve the UI | `init` · `triage` · `draft` · `report` · `check` · `serve` | just the `provreq` binary + the subject's source tree                            | **Yes — native on any OS**             |
| **Executor** — actually run a verification engine                  | `verify` (`engines` to probe first)                        | the **engine** _and_ the **built** subject / a recorded trace / a running system | **No — runs where the subject builds** |

The brain is a self-contained binary that touches only files; it runs natively on Windows,
macOS, or Linux with nothing else installed. The executor is the part that needs the
Linux-centric engine ecosystem (Creusot/Why3, Prusti, MonPoly, TLC's JRE, a Selenium grid),
and it needs the subject _already built_. So the cross-platform answer falls out cleanly:

> **Run the brain natively wherever you like; run verification wherever the subject builds.**
> For a containerized subject like qrusty, that is _inside its container_.

## Getting the `provreq` binary

Three channels, in preference order.

### 1. Prebuilt release binary (intended primary path)

[build-and-release.md](build-and-release.md) defines a six-target matrix
(`{x86_64,aarch64}` × `{linux-gnu, pc-windows-msvc, apple-darwin}`) published as a GitHub
Release on a `v*` tag, each asset carrying a `.sha256` and listed in `dist-manifest.json`.
Read your host triple, pick the matching asset, verify the sha256, put the binary on `PATH`.

> ⚠️ **No `v*` release has been cut yet.** The pipeline (`.github/workflows/release.yml`)
> exists but has never fired, so there is nothing to download today. Until the first tag is
> pushed, use build-from-source below. Cutting a release is just `git tag v0.x.y` then a
> push of the tag — the CI/CD does the rest — but it is a deliberate act, not an accident.

### 2. Build from source (the working path today, and the always-available fallback)

Requires a Rust toolchain and Node (for the embedded UI). The frontend **must** be built
before `cargo`, because `rust-embed` bakes `web/dist` into the binary:

```sh
git clone https://github.com/greeng3/provablerequirements
cd provablerequirements
make web                       # npm ci + vite build → web/dist  (required before cargo)
cargo build --release          # → target/release/provreq
# or, to put it on PATH:
cargo install --path .         # (still needs `make web` to have run first)
```

`make build` / `make test` already depend on `make web`, so those do the right thing. A bare
`cargo build` without `make web` compiles, but `serve` will show only the "UI not built yet"
placeholder — fine for CLI-only use, wrong if you want the web UI.

### 3. Bake it into the subject's container image

For a containerized subject, add provreq to _that image_ — either `COPY` a release binary in,
or build from source in a stage — so it is present the moment the dev container comes up. This
is the qrusty path; see the walkthrough below. This is **holding the line**: provreq goes
_into_ the subject's environment. We deliberately do **not** publish a runnable provreq
container image — one would invite the "provreq builds the subject" anti-pattern A5 forbids.

## Engines: per-category, optional, and honestly reported

provreq never installs an engine behind your back and never pretends one is present.
`provreq engines` probes what is installed (a `PATH` lookup, or a `GET <endpoint>/status` for
a Selenium grid) and reports, per category, what is therefore checkable. A missing engine is
`Missing` — yours to install; a wired-but-broken one is `Unusable` — ours to fix.

| Category                       | Engine(s)             | Shape of the dependency                                               |
| ------------------------------ | --------------------- | --------------------------------------------------------------------- |
| 1 — code (deductive / bounded) | Kani, Creusot, Prusti | Rust toolchain + provers; heaviest to install                         |
| 2a — model checking            | TLC                   | a single `tla2tools.jar` under a JRE (`TLA2TOOLS_JAR`) — light        |
| 2b — runtime monitor           | MonPoly               | an OCaml-built binary; reads a recorded jsonl trace the subject emits |
| 3 — UI                         | Selenium              | a WebDriver **service** (port 4444), not a `PATH` binary              |

provreq can bootstrap two of these itself: **`provreq install tlc`** and **`provreq install
kani`** (with `--yes` to consent to the download + write). For engines it will not install
natively, `provreq install <engine>` instead _explains_ what that engine needs in terms of
the subject's own build environment (REQ048).

For the full set installed into one Linux environment, this repo's own
[`.devcontainer/Dockerfile`](../.devcontainer/Dockerfile) is the **reference recipe** — it
bakes in Kani, TLC, Prusti, Creusot, MonPoly, and a Selenium/Chrome grid, with the exact
versions, env vars, and the traps that matter (e.g. Prusti's `PRUSTI_*` env-var landmine).
Reuse the stanzas your subject's categories actually need rather than copying the whole file.

**Verification runs in a Linux environment — use Docker for now.** The engine ecosystem is
Linux-centric, so on macOS or Windows the executor lives in a container (the subject's dev
container, a plain Docker image, or WSL2 on Windows). Native Windows/macOS engine support is
**future work**, tracked with the release-signing gaps in build-and-release.md.

## Per-environment quickstart

Every row is the same two ingredients — the binary, plus engines _only if you will `verify`_.

- **Linux (native).** Install the binary (channel 1 or 2). For verification, install the
  engines your categories need (or reuse the devcontainer recipe). Brain commands need
  nothing further.
- **macOS.** Run the brain natively — `triage`, `draft`, `report`, `serve` all work with just
  the binary. Unsigned release binaries hit Gatekeeper (right-click → Open, or
  `xattr -d com.apple.quarantine`). Run `verify` inside Docker, against the subject's build.
- **Windows.** Same: brain runs natively (the MSVC build); SmartScreen will warn on an
  unsigned binary. Run `verify` inside WSL2 or Docker, where the Linux engines and the
  subject's Linux build live.
- **Docker / dev container (the primary case).** provreq lives in the same container as the
  subject's build and engines; `verify` is a local subprocess call. This is A5-Option-B and
  the home of the CLI walking skeleton.

## qrusty walkthrough

qrusty has its own dev container and build. We do **not** build qrusty from here — provreq
goes to qrusty, not the reverse.

1. **Put provreq in qrusty's dev container.** Add it to qrusty's `.devcontainer` image
   (channel 3): `COPY` a release binary once one exists, or build from source in a stage
   today. Add the engine stanzas qrusty's categories need — per
   [qrusty-as-a-subject.md](qrusty-as-a-subject.md) the weight is **2a (TLC), 2b (MonPoly),
   and 3 (Selenium)**, _not_ the Creusot/Prusti stack — so a JRE + `tla2tools.jar`, a MonPoly
   binary, and a reachable WebDriver endpoint, not the heavy deductive tooling.
2. **Adopt.** Inside the container: `provreq init /workspaces/qrusty` discovers qrusty's
   Doorstop layout and proposes a peer companion tree (operator-confirmed). This scaffolds
   over all six of qrusty's documents.
3. **Probe.** `provreq engines /workspaces/qrusty` — confirm 2a/2b/3 read as present before
   expecting verdicts.
4. **Work the backlog.** `provreq triage`, then `provreq draft`, then `provreq serve --path
/workspaces/qrusty --port 17869` for the UI. (17869 is provreq's default; 17867 is qrusty's
   own devcontainer port — they do not collide.)

Throughout, the trust boundary from A6 holds: provreq stages proof carriers and back-links as
**uncommitted working-tree edits** in the checked-out subject and stops there. It never runs
git in qrusty, holds no commit or push rights, and makes no forge assumption — you review the
diff and open the merge request on GitLab yourself. `provreq verify` writes `verdicts.yml`
into the companion tree; expected once qrusty is deliberately adopted, surprising before that.

## Related

- [build-and-release.md](build-and-release.md) — how the binaries are built and published (producer side).
- [applying-to-existing-repos.md](applying-to-existing-repos.md) — where artifacts live, the brain/executor seam (A5), the write-through-review boundary (A6).
- [qrusty-as-a-subject.md](qrusty-as-a-subject.md) — what qrusty's tree contains and which categories carry the weight.
- [`.devcontainer/Dockerfile`](../.devcontainer/Dockerfile) — the reference engine-install recipe.
