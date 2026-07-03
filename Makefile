# Makefile — formatting, linting, and requirements checks.
#
# Scoped to what the repo has today: Markdown, YAML, and Doorstop requirements.
# Rust and Node/React targets will be added when implementation starts (see
# .devcontainer/README.md). Modelled on the qrusty Makefile.
#
# All tools are provided by the dev container (.devcontainer) — run inside it.

.DEFAULT_GOAL := help

MARKDOWNLINT := markdownlint-cli2
PRETTIER     := prettier
YAMLLINT     := yamllint
DOORSTOP     := doorstop
TRACEABILITY := uv run scripts/traceability.py
TRACE_REPORT := docs/traceability_report.md

# Prettier ignores come from .prettierignore; markdownlint/yamllint exclusions
# are passed explicitly below. The qrusty symlink and generated dirs are always
# excluded so we never lint/format outside this repo.

.PHONY: help \
	fmt fmt-check lint pre-merge setup-hooks check-requirements \
	fmt-md fmt-check-md lint-md \
	fmt-yaml fmt-check-yaml lint-yaml \
	traceability traceability-report traceability-check

help:
	@echo "Targets:"
	@echo "  fmt                 Format Markdown + YAML (prettier --write)"
	@echo "  fmt-check           Check formatting without writing"
	@echo "  lint                Lint Markdown (markdownlint) + YAML (yamllint)"
	@echo "  check-requirements  Validate the Doorstop requirements tree"
	@echo "  traceability        Print a requirements traceability report"
	@echo "  traceability-report Write the report to $(TRACE_REPORT)"
	@echo "  traceability-check  Fail if any code tag references an unknown requirement"
	@echo "  pre-merge           Preflight: traceability, fmt, fmt-check, lint, requirements"
	@echo "  setup-hooks         Install git hooks (core.hooksPath -> .githooks)"
	@echo ""
	@echo "  Per-language: fmt-md fmt-check-md lint-md  fmt-yaml fmt-check-yaml lint-yaml"

# --- aggregates ---
fmt: fmt-md fmt-yaml
fmt-check: fmt-check-md fmt-check-yaml
lint: lint-md lint-yaml

# --- Markdown ---
fmt-md:
	@$(PRETTIER) --write --ignore-unknown --ignore-path .prettierignore "**/*.md"

fmt-check-md:
	@$(PRETTIER) --check --ignore-unknown --ignore-path .prettierignore "**/*.md"

lint-md:
	@$(MARKDOWNLINT) "**/*.md" "!qrusty/**" "!requirements-doorstop/**" "!**/node_modules/**" "!.venv/**"

# --- YAML ---
fmt-yaml:
	@$(PRETTIER) --write --ignore-unknown --ignore-path .prettierignore "**/*.yml" "**/*.yaml"

fmt-check-yaml:
	@$(PRETTIER) --check --ignore-unknown --ignore-path .prettierignore "**/*.yml" "**/*.yaml"

lint-yaml:
	@$(YAMLLINT) -c .yamllint.yml .

# --- Requirements ---
check-requirements:
	@$(DOORSTOP)

traceability:
	@$(TRACEABILITY)

traceability-report:
	@$(TRACEABILITY) --output $(TRACE_REPORT)

traceability-check:
	@$(TRACEABILITY) --check >/dev/null

# --- Preflight ---
pre-merge:
	@echo "=== pre-merge preflight ==="
	@echo "Manual prerequisite (not automated): update docs and requirements for the changes being merged."
	@echo ""
	@echo "[1/6] traceability-report (regenerate $(TRACE_REPORT))"
	@$(MAKE) --no-print-directory traceability-report
	@echo "[2/6] fmt"
	@$(MAKE) --no-print-directory fmt
	@echo "[3/6] fmt-check"
	@$(MAKE) --no-print-directory fmt-check
	@echo "[4/6] lint"
	@$(MAKE) --no-print-directory lint
	@echo "[5/6] check-requirements"
	@$(MAKE) --no-print-directory check-requirements
	@echo "[6/6] traceability-check (no orphan tags)"
	@$(MAKE) --no-print-directory traceability-check
	@echo ""
	@echo "=== pre-merge passed ==="
	@echo "(Rust and Node/React targets to be added as those land.)"

setup-hooks:
	@git config core.hooksPath .githooks
	@echo "Installed: core.hooksPath -> .githooks (pre-commit runs fmt-check, lint, check-requirements)"
