# Build & Release Pipeline

How the PRL native executable is built and published. Realises requirements
REQ001–REQ004 and the packaging half of **Design C** (see
[operator-workflow-notes.md](operator-workflow-notes.md)). Tracked in GitHub
issue #2.

> **Status: prototype.** The crate is a skeleton (`provreq`, a health/version
> binary) — just enough for the pipeline to have a real target to build and
> publish. Real server routes and the embedded web UI are a follow-up.

## Why a native executable

Design C distributes the tool as a native, self-contained executable the
operator runs in their own dev environment (REQ001). That means we ship
**prebuilt binaries per platform**, so the operator never compiles the tool
itself — which in turn means CI has to cross-build the full platform matrix.

## Supported target matrix (REQ002)

Six targets — three OSes × two architectures — each pinned to a runner:

| Target triple                | Runner            |
| ---------------------------- | ----------------- |
| `x86_64-unknown-linux-gnu`   | `ubuntu-latest`   |
| `aarch64-unknown-linux-gnu`  | `ubuntu-24.04-arm`|
| `x86_64-pc-windows-msvc`     | `windows-latest`  |
| `aarch64-pc-windows-msvc`    | `windows-latest`  |
| `x86_64-apple-darwin`        | `macos-13`        |
| `aarch64-apple-darwin`       | `macos-14`        |

macOS and Windows need real runners — macOS cannot be built off Apple hardware,
and MSVC targets are impractical to cross-build. This is the whole reason the
repo lives on **public GitHub**: unlimited free Actions minutes across all three
OSes. The supported set is explicit and finite, not auto-discovered (REQ002).

## Workflows

- **`.github/workflows/ci.yml`** — on push to `main` and every PR: `cargo fmt
  --check`, `cargo clippy -D warnings`, `cargo test`. Fast, host-only.
- **`.github/workflows/release.yml`** — on a version tag (`v*`): builds all six
  targets, packages each (`.tar.gz` on unix, `.zip` on Windows) with a `.sha256`
  sidecar, then a `release` job assembles the manifest and publishes a GitHub
  Release with every artifact attached (REQ003, REQ004).

## Release manifest (REQ003)

Each release includes `dist-manifest.json` so the provisioner can pick the right
asset per platform without scraping the releases page:

```json
{
  "schema": 1,
  "version": "0.1.0",
  "artifacts": [
    {
      "target": "x86_64-unknown-linux-gnu",
      "file": "provreq-0.1.0-x86_64-unknown-linux-gnu.tar.gz",
      "sha256": "<hex>"
    }
  ]
}
```

The provisioner reads its own host triple, finds the matching `target`, downloads
`file` from the release, and verifies `sha256` before running it.

## Why hand-rolled, not cargo-dist (for now)

[`cargo-dist`](https://opensource.axo.dev/cargo-dist/) automates exactly this and
remains a reasonable future swap. For the prototype a hand-written matrix is
preferred: ~100 lines we fully control, no extra build-time tool to install and
pin, and a manifest schema we own rather than adapt to. If the release surface
grows (installers, updaters, signing), revisit cargo-dist.

## Not yet covered

- **Code signing / notarization** — unsigned macOS binaries hit Gatekeeper and
  unsigned Windows hits SmartScreen. Fine for a prototype (bypassable); real
  distribution needs an Apple Developer cert + notarization and a Windows
  Authenticode cert.
- **Cross-execution tests** — the release workflow only *builds* the non-host
  targets (e.g. `aarch64-pc-windows-msvc` on an x64 runner). Behavioural tests
  run host-only in `ci.yml`.
