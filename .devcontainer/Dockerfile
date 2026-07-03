# Base image shared with the qrusty devcontainer, so the two projects share a
# common toolchain foundation. (Ubuntu-based rust-dev image.)
FROM greeng340or/rust-dev-ubuntu

# Download tools + certificates needed by the install steps below.
RUN apt-get update \
    && apt-get install -y curl ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# GitLab CLI (glab) — used constantly in this repo for issues and merge requests.
# Installed from the official release .deb (apt resolves any dependencies).
ARG GLAB_VERSION=1.106.0
RUN curl -fsSL -o /tmp/glab.deb \
      "https://gitlab.com/gitlab-org/cli/-/releases/v${GLAB_VERSION}/downloads/glab_${GLAB_VERSION}_linux_amd64.deb" \
    && apt-get update \
    && apt-get install -y /tmp/glab.deb \
    && rm -f /tmp/glab.deb \
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
    && ln -s /opt/doorstop/bin/doorstop /usr/local/bin/doorstop \
    && ln -s /opt/doorstop/bin/doorstop-server /usr/local/bin/doorstop-server \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# uv (fast Python package/venv manager) — for running project Python scripts
# (e.g. requirements traceability) and managing their dependencies, mirroring
# the qrusty devcontainer. Relies on the distro python3 installed above.
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"
