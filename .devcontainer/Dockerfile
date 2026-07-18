# Base image shared with the qrusty devcontainer, so the two projects share a
# common toolchain foundation. (Ubuntu-based rust-dev image.)
FROM greeng340or/rust-dev-ubuntu

# Download tools + certificates needed by the install steps below.
RUN apt-get update \
    && apt-get install -y curl ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# GitLab CLI (glab) — used constantly in this repo for issues and merge requests.
# Installed from the official release .deb (apt resolves any dependencies). The
# architecture is detected so the image builds on both amd64 and arm64 hosts;
# `dpkg --print-architecture` yields amd64/arm64, matching glab's .deb naming.
ARG GLAB_VERSION=1.107.0
RUN arch="$(dpkg --print-architecture)" \
    && curl -fsSL -o /tmp/glab.deb \
      "https://gitlab.com/gitlab-org/cli/-/releases/v${GLAB_VERSION}/downloads/glab_${GLAB_VERSION}_linux_${arch}.deb" \
    && apt-get update \
    && apt-get install -y /tmp/glab.deb \
    && rm -f /tmp/glab.deb \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI (gh) — for the move to a public GitHub repo (issues, PRs, releases).
# Same pattern as glab above: official release .deb, architecture detected so the
# image builds on amd64 and arm64 hosts (`dpkg --print-architecture` yields
# amd64/arm64, matching gh's .deb naming).
ARG GH_VERSION=2.96.0
RUN arch="$(dpkg --print-architecture)" \
    && curl -fsSL -o /tmp/gh.deb \
      "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${arch}.deb" \
    && apt-get update \
    && apt-get install -y /tmp/gh.deb \
    && rm -f /tmp/gh.deb \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Doorstop (requirements management) and its prerequisites.
# We manage requirements as Doorstop items (YAML under git) alongside the
# Markdown design docs. Installed into an isolated venv so it doesn't disturb
# the system Python; the CLI and web-server entry points are exposed on PATH.
# Doorstop keeps items under version control, so it relies on git (already in
# the base image).
RUN apt-get update \
    && apt-get install -y python3 python3-venv \
    && python3 -m venv /opt/doorstop \
    && /opt/doorstop/bin/pip install --upgrade pip \
    && /opt/doorstop/bin/pip install doorstop \
    && ln -sf /opt/doorstop/bin/doorstop /usr/local/bin/doorstop \
    && ln -sf /opt/doorstop/bin/doorstop-server /usr/local/bin/doorstop-server \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# uv (fast Python package/venv manager) — for running project Python scripts
# (e.g. requirements traceability) and managing their dependencies, mirroring
# the qrusty devcontainer. Relies on the distro python3 installed above.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"

# yamllint — YAML linting for the `make lint-yaml` / `pre-merge` targets.
RUN uv tool install yamllint

# cargo-audit — scans the dependency tree for known CVEs (RustSec) in the
# `make audit` / `pre-merge` targets. Justified by the HTTP/TLS stack (reqwest,
# rustls) the LLM classifier pulls in. cargo comes from the base rust-dev image.
RUN cargo install cargo-audit --locked

# Kani — the category-1 verification engine (REQ027). Baked into the IMAGE, not just
# installed at runtime: the repo bakes in every workflow tool (cargo-audit above is the
# precedent), a runtime-only install evaporates on rebuild, and then `provreq engines`
# reports category 1 Missing — indistinguishable from a bug in our own code. Two steps:
# `cargo install` lays down the driver; `cargo kani setup` fetches CBMC + the SAT/SMT
# solvers it drives, and that second step is what grows the image (hundreds of MB).
RUN cargo install --locked kani-verifier \
    && cargo kani setup

# TLC — the category-2a verification engine (REQ029), for the model (TLA+) world. Same reason
# as Kani above: baked into the image so `provreq engines` never reports 2a Missing on a fresh
# rebuild. TLC ships as a single jar run under a JRE (no PATH binary), and is far lighter than
# Kani/CBMC — a headless JRE plus a ~2 MB jar. `TLA2TOOLS_JAR` is the path `provreq::tlc` reads.
ENV TLA2TOOLS_JAR=/opt/tlaplus/tla2tools.jar
RUN apt-get update \
    && apt-get install -y --no-install-recommends default-jre-headless \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /opt/tlaplus \
    && curl -fLsS -o "$TLA2TOOLS_JAR" \
      "https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar" \
    && java -cp "$TLA2TOOLS_JAR" tlc2.TLC 2>&1 | grep -q "TLC2 Version"

# Prusti — the SECOND category-1 verification engine (deductive; the rung that earns `proven`,
# alongside Kani's bounded `model-checked` — D2b's ensemble). Baked in for the same reason as
# Kani/TLC: a runtime install evaporates on rebuild and then `provreq engines` reports category 1
# short an engine, indistinguishable from a bug in our own code.
#
# Unlike Kani/TLC there is no release binary for this arch, so Prusti is SOURCE-BUILT at a pinned
# tag. Its verifier backend is Viper (arch-independent JVM bytecode via the viper_tools jars that
# `x.py setup` fetches); only the Rust driver compiles natively. Two things make that build work
# here, both non-obvious:
#   1. `RUSTFLAGS=-C prefer-dynamic` — prusti-driver is itself a rustc driver, so it links the
#      rustc-private crates (rustc_middle, rustc_driver, …). This pinned nightly's `rustc-dev`
#      ships those crates as dylibs ONLY, so a default (static-rlib) link fails with ~196
#      "required to be available in rlib format" errors; prefer-dynamic links the .so form on disk.
#   2. System `z3` (apt) — the z3 bundled in viper_tools is an x86_64 binary and rosetta-fails on
#      arm64, so `Z3_EXE` points at the distro-native z3 instead.
# The bins land under target/<host-triple>/release/ (the repo's .cargo/config sets bindeps, which
# forces an explicit target triple), so the path is computed, never hardcoded. The pinned nightly
# toolchain is left installed on purpose: `cargo prusti` recompiles the subject with the prusti
# driver and needs it at verify time. The JRE from the TLC stage above doubles as Prusti's JVM.
ARG PRUSTI_TAG=v-2023-08-22-1715
ENV JAVA_HOME=/usr/lib/jvm/default-java \
    VIPER_HOME=/opt/prusti-src/viper_tools/backends \
    Z3_EXE=/usr/bin/z3 \
    PRUSTI_BIN=/opt/prusti/bin
ENV PATH="${PRUSTI_BIN}:${PATH}"
# `x.py setup` shells out to `sudo apt-get` to install its native build deps (it detects
# Ubuntu and runs setup_ubuntu), so `sudo` must exist even though the Docker build is already
# root — a clean base image has no sudo. We also pre-install those deps here so the stage is
# self-sufficient; `x.py setup` re-running apt with them present is a harmless no-op.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      z3 git python3 sudo build-essential pkg-config curl gcc libssl-dev unzip \
    && rm -rf /var/lib/apt/lists/* \
    && git clone --depth 1 --branch "$PRUSTI_TAG" \
      https://github.com/viperproject/prusti-dev /opt/prusti-src \
    && cd /opt/prusti-src \
    && python3 ./x.py setup \
    && LD_LIBRARY_PATH="${JAVA_HOME}/lib/server:${JAVA_HOME}/lib" \
       RUSTFLAGS="-C prefer-dynamic" \
       cargo build --release -p prusti -p prusti-launch -p prusti-server \
    && host="$(rustc -vV | sed -n 's/^host: //p')" \
    && mkdir -p "$(dirname "$PRUSTI_BIN")" \
    && ln -sfn "/opt/prusti-src/target/${host}/release" "$PRUSTI_BIN" \
    && test -x "${PRUSTI_BIN}/cargo-prusti" \
    && test -x "${PRUSTI_BIN}/prusti-rustc" \
    && test -x "${PRUSTI_BIN}/prusti-driver"
