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
