# Dev Container

A [Dev Container](https://containers.dev/) providing a consistent environment for working
on this repo.

## What's inside

- **Base image:** `greeng340or/rust-dev-ubuntu` — the same base as the companion `qrusty`
  devcontainer, so the two projects share a toolchain foundation.
- **`gh`** (GitHub CLI) — used throughout for issues, pull requests, and releases.
  (`glab` is also installed, a leftover from the repo's GitLab era.)
- **Node LTS** (via a devcontainer feature) + **`markdownlint-cli2`**, **`prettier`**,
  and **`yamllint`** — Markdown/YAML linting and formatting matching the editor rules,
  so docs lint the same way in the editor and on the command line.
- **Doorstop** — requirements management (items stored as YAML under git). We manage
  requirements as Doorstop items alongside the Markdown design docs. Installed in an
  isolated venv (`/opt/doorstop`); the `doorstop` CLI and `doorstop-server` are on PATH.
- **Verification engines, baked into the image** so `provreq engines` never reports a
  category missing on a fresh container: **Kani** (cargo kani + CBMC), **TLC**
  (`tla2tools.jar`, path in `TLA2TOOLS_JAR`), **Prusti** (source-built at a pinned
  commit), and **Creusot** (installed via `creusot-setup`, with two local patches — an
  arm64 provers table and the coroutine-ICE fix, both documented in the Dockerfile).
  MonPoly (category 2b) and a Selenium grid (category 3) are external — MonPoly builds
  from source on demand; Selenium runs as a service on port 4444, not a PATH binary.
- **ReqForge's gate tools** — `cargo-llvm-cov` (with the `llvm-tools-preview` component),
  `cargo-outdated`, and `taplo`. Phase 2 of the ReqForge absorb moves its code into this
  repo rather than extracting a model crate in its own tree, so this image has to be able
  to run its `make pre-merge`. `ruff` and `mypy` are deliberately absent: its Makefile
  invokes them through `uv tool run`, and `uv` is already here. See issue #299 for what
  was measured, including the packages in ReqForge's Dockerfile that nothing in its
  dependency graph actually needs.
- Curated VS Code extensions (Markdown, YAML, TOML, TODO tree, GitHub PRs, GitHub
  Actions, Claude Code).

## Usage

1. Install Docker and VS Code with the **Dev Containers** extension
   (`ms-vscode-remote.remote-containers`).
2. Open this repo in VS Code and run **"Dev Containers: Reopen in Container"**.
3. On first build the container prints a tool-versions summary (git, glab, node,
   markdownlint-cli2, prettier, yamllint, doorstop, kani, TLC) so you can confirm it's
   healthy — or run `make check-tools` any time.

Lint the docs from inside the container with:

```sh
markdownlint-cli2 "**/*.md"
```

## Doorstop requirements

Doorstop stores requirements as version-controlled YAML items. Common commands:

```sh
doorstop                       # validate the requirements tree
doorstop create REQ ./reqs     # create a document (once, when starting the tree)
doorstop add REQ               # add a new requirement item
doorstop edit REQ001           # edit an item
doorstop publish REQ ./out     # publish to text/markdown/html
```

The requirements tree lives at [`requirements-doorstop/`](../requirements-doorstop) (prefix
`REQ`). To browse requirements in a web UI, start the server (port `17868` is forwarded —
chosen to avoid clashing with the qrusty devcontainer's `17867`):

```sh
doorstop-server --host 0.0.0.0 --port 17868
```

## Phase 2 — done

An earlier revision of this file planned a "Phase 2" of additions. All of it has since
landed: `scripts/traceability.py` traces Doorstop items to code, the Makefile carries
`fmt` / `lint` / `pre-merge` / `setup-hooks` (see `make help`), the Rust and React
toolchains are wired in, and the verification engines are baked into the image as
described above.
