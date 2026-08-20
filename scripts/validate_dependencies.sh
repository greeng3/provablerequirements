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
    if ! command -v "$cmd" >/dev/null 2>&1; then
        printf '  MISSING  %-16s (not on PATH)\n' "$label"
        status=1
        return
    fi
    # On PATH is not the same as runnable. Capture the exit status of the tool itself, not of the
    # `head` it used to be piped into: a cargo subcommand invoked with the wrong arguments prints
    # an error and exits non-zero, and this printed `ok` beside that error text until #299.
    if ! out="$("$cmd" "$@" 2>&1)"; then
        printf '  BROKEN   %-16s %s\n' "$label" "$(printf '%s' "$out" | head -n1)"
        status=1
        return
    fi
    printf '  ok       %-16s %s\n' "$label" "$(printf '%s' "$out" | head -n1)"
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
# cargo-llvm-cov is a cargo SUBCOMMAND: invoked directly it demands its own name back
# before any flag, so a bare `--version` is an argument error rather than a version.
check cargo-llvm-cov    cargo-llvm-cov llvm-cov --version  # ReqForge's coverage gate (#299)
check cargo-outdated    cargo-outdated outdated --version  # ReqForge's audit-deps gate (#299)
check taplo             taplo         --version   # ReqForge's TOML fmt/lint gates (#299)
echo

if [ "$status" -eq 0 ]; then
    echo "All required tools present."
else
    echo "Some tools are missing — rebuild/open the dev container (.devcontainer)."
fi

exit "$status"
