# Dev Container

A [Dev Container](https://containers.dev/) providing a consistent environment for working
on this repo (see issue #3).

## What's inside

- **Base image:** `greeng340or/rust-dev-ubuntu` — the same base as the companion `qrusty`
  devcontainer, so the two projects share a toolchain foundation.
- **`glab`** (GitLab CLI) — used throughout for issues and merge requests.
- **Node LTS** (via a devcontainer feature) + **`markdownlint-cli2`** — Markdown linting
  matching the `DavidAnson.vscode-markdownlint` editor rules, so docs lint the same way in
  the editor and on the command line.
- **Doorstop** — requirements management (items stored as YAML under git). We manage
  requirements as Doorstop items alongside the Markdown design docs. Installed in an
  isolated venv (`/opt/doorstop`); the `doorstop` CLI and `doorstop-server` are on PATH.
- Curated VS Code extensions (Markdown, YAML, TOML, TODO tree, GitLab Workflow, Claude
  Code).

## Usage

1. Install Docker and VS Code with the **Dev Containers** extension
   (`ms-vscode-remote.remote-containers`).
2. Open this repo in VS Code and run **"Dev Containers: Reopen in Container"**.
3. On first build the container prints a tool-versions summary
   (`git`, `glab`, `node`, `markdownlint-cli2`) so you can confirm it's healthy.

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

## Extending it (Phase 2)

The base is kept lean. Planned additions, mostly modelled on the qrusty devcontainer and
its Makefile:

- **Requirements traceability** — adapt qrusty's `scripts/traceability.py` and
  `scripts/validate_dependencies.sh` to trace between Doorstop items and code/docs. `uv`
  is already installed for running these (and other) project Python scripts.
- **A Makefile** with `fmt` / `fmt-check` / `lint` (and per-language `-rust` / `-py` /
  `-node` / `-md` / `-yaml`) plus a **`pre-merge`** aggregate target and `setup-hooks`,
  like qrusty.
- **Rust** — already provided by the base image (`rust-dev-ubuntu`); add `fmt-rust` /
  `lint-rust` (rustfmt + clippy) targets when coding starts.
- **React web UI** (if built) — Node is already present; add the React toolchain plus
  Prettier/ESLint and matching `fmt-node` / `lint-node` targets.
- **Verification toolchain** — an SMT solver such as Z3, and IVL / model-checking tooling
  (Viper, TLA+, MonPoly) per `docs/requirement-language.md`, added behind optional layers
  or devcontainer features rather than bloating the base image.
