#!/usr/bin/env bash
#
# Validate that the dev toolchain this repo depends on is present and runnable.
# These tools are provided by the dev container (.devcontainer) — run inside it.
#
# Covers the docs/requirements workflow plus the Rust toolchain the product now
# uses. Node/React product checks will be added here when that code lands,
# mirroring how the Makefile grows with the stack.

set -u

status=0

# check <command> <label> [version-args...]
check() {
    cmd="$1"
    label="$2"
    shift 2
    if command -v "$cmd" >/dev/null 2>&1; then
        ver="$("$cmd" "$@" 2>&1 | head -n1)"
        printf '  ok       %-16s %s\n' "$label" "$ver"
    else
        printf '  MISSING  %-16s (not on PATH)\n' "$label"
        status=1
    fi
}

echo "=== dev toolchain ==="
check git               git           --version
check glab              glab          --version   # GitLab CLI (issues / MRs)
check doorstop          doorstop      --version   # requirements management
check uv                uv            --version   # runs project Python scripts
check python3           python3       --version
check node              node          --version   # powers the Node linters/formatters
check npm               npm           --version
check markdownlint-cli2 markdownlint  --version   # make lint-md
check prettier          prettier      --version   # make fmt / fmt-check
check yamllint          yamllint      --version   # make lint-yaml
check cargo             cargo         --version   # Rust build / test / clippy
check cargo-audit       cargo-audit   --version   # make audit (dependency CVEs)
echo

if [ "$status" -eq 0 ]; then
    echo "All required tools present."
else
    echo "Some tools are missing — rebuild/open the dev container (.devcontainer)."
fi

exit "$status"
