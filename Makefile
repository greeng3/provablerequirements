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

# Prettier ignores come from .prettierignore; markdownlint/yamllint exclusions
# are passed explicitly below. The qrusty symlink and generated dirs are always
# excluded so we never lint/format outside this repo.

.PHONY: help \
	fmt fmt-check lint pre-merge setup-hooks check-requirements \
	fmt-md fmt-check-md lint-md \
	fmt-yaml fmt-check-yaml lint-yaml

help:
	@echo "Targets:"
	@echo "  fmt                 Format Markdown + YAML (prettier --write)"
	@echo "  fmt-check           Check formatting without writing"
	@echo "  lint                Lint Markdown (markdownlint) + YAML (yamllint)"
	@echo "  check-requirements  Validate the Doorstop requirements tree"
	@echo "  pre-merge           Preflight: fmt, fmt-check, lint, check-requirements"
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

# --- Preflight ---
pre-merge:
	@echo "=== pre-merge preflight ==="
	@echo "Manual prerequisite (not automated): update docs and requirements for the changes being merged."
	@echo ""
	@echo "[1/4] fmt"
	@$(MAKE) --no-print-directory fmt
	@echo "[2/4] fmt-check"
	@$(MAKE) --no-print-directory fmt-check
	@echo "[3/4] lint"
	@$(MAKE) --no-print-directory lint
	@echo "[4/4] check-requirements"
	@$(MAKE) --no-print-directory check-requirements
	@echo ""
	@echo "=== pre-merge passed ==="
	@echo "(Rust, Node/React, coverage, and traceability steps to be added as those land.)"

setup-hooks:
	@git config core.hooksPath .githooks
	@echo "Installed: core.hooksPath -> .githooks (pre-commit runs fmt-check, lint, check-requirements)"
