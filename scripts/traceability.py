#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Requirements traceability report generator.

Scans source files for requirement tags in comments and cross-references them
against the ReqForge requirements project so we can see which requirements are
implemented / verified, which are uncovered, and which tags reference a
requirement that does not exist (orphans).

The valid requirement ids are the artifact filename stems under
`requirements/artifacts/<collection>/<id>.md` — the same id provreq reads. No
Doorstop dependency: provreq's requirements moved to ReqForge (#321), and this
only needs the id set, which the filenames already carry. (Longer term provreq
itself will own traceability; this is the interim reader.)

Tagging conventions (in code comments, any supported language):

    // Implements: REQ001, REQ002      (or `Requirements:`)
    // Verifies:   REQ003

Doc comments (`///` in Rust) work too. IDs are matched case-insensitively and a
`-`/`_` separator is optional, so `REQ001`, `REQ-001`, and `req_001` are equal.

Run with uv:

    uv run scripts/traceability.py                       # markdown to stdout
    uv run scripts/traceability.py --output docs/traceability_report.md
    uv run scripts/traceability.py --format json
    uv run scripts/traceability.py --check               # exit 1 on orphan tags
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

DEFAULT_SRC_DIRS = ["src", "tests", "crates", "web_ui", "web-ui"]
DEFAULT_REQUIREMENTS_ROOT = "requirements"
DEFAULT_PREFIXES = {"REQ"}

SOURCE_EXTS = {".rs", ".ts", ".tsx", ".js", ".py"}
IGNORE_DIRS = {
    ".git", ".venv", "venv", "node_modules", "target", "dist", "build",
    "__pycache__", "qrusty", "requirements", "docs", "scripts",
    ".devcontainer", ".githooks",
}

TAG_PATTERNS = [
    (re.compile(r"(?:Requirements?|Implements?):\s*([\w\-,\s]+)", re.I), "implements"),
    (re.compile(r"Verifies?:\s*([\w\-,\s]+)", re.I), "verifies"),
]
ID_SPLIT = re.compile(r"[,\s]+")
ID_RE = re.compile(r"^(?P<prefix>[A-Za-z]+)[-_]?(?P<num>\d+)$")


def canonical(uid: str) -> str:
    """Normalize a requirement id for comparison (REQ-001 == REQ001 == req_001)."""
    return uid.upper().replace("-", "").replace("_", "")


@dataclass
class Location:
    file: str
    line: int
    context: str
    trace_type: str  # "implements" | "verifies"


@dataclass
class Trace:
    uid: str            # canonical id
    display: str        # original Doorstop uid if known, else the canonical id
    text: str | None = None
    known: bool = False  # exists in the Doorstop tree
    implementations: list[Location] = field(default_factory=list)
    verifications: list[Location] = field(default_factory=list)


def load_requirements(root: Path) -> dict[str, Trace]:
    """Load requirement ids from the ReqForge project's artifact filenames, keyed by canonical id.

    Each `requirements/artifacts/<collection>/<id>.md` names one requirement; the filename stem is
    the id provreq reads. Operating-system resource files (`.DS_Store`, `._*` AppleDouble sidecars)
    are never requirements, so they are skipped — a `._REQ001.md` sidecar carries the `.md` suffix
    but is not a second declaration of `REQ001`.
    """
    reqs: dict[str, Trace] = {}
    artifacts = root / "artifacts"
    if not artifacts.is_dir():
        print(
            f"Warning: no ReqForge artifacts under {artifacts}; reporting code tags only.",
            file=sys.stderr,
        )
        return reqs
    for md in sorted(artifacts.glob("*/*.md")):
        name = md.name
        if name == ".DS_Store" or name.startswith("._"):
            continue
        uid = md.stem
        reqs[canonical(uid)] = Trace(
            uid=canonical(uid), display=uid, text=_artifact_prose(md), known=True
        )
    return reqs


def _artifact_prose(md: Path) -> str | None:
    """The artifact body (the Markdown after the JSON frontmatter), truncated for the report."""
    try:
        raw = md.read_text()
    except OSError as exc:
        print(f"Warning: could not read {md}: {exc}", file=sys.stderr)
        return None
    body = raw
    if raw.startswith("---\n"):
        rest = raw[4:]
        end = rest.find("\n---\n")
        if end != -1:
            body = rest[end + len("\n---\n") :]
    body = body.strip()
    if not body:
        return None
    return body[:100] + "…" if len(body) > 100 else body


def valid_prefixes(reqs: dict[str, Trace]) -> set[str]:
    """Requirement prefixes to recognize in code — derived from the tree, or the default."""
    prefixes = set()
    for trace in reqs.values():
        match = ID_RE.match(trace.uid)
        if match:
            prefixes.add(match.group("prefix").upper())
    return prefixes or set(DEFAULT_PREFIXES)


def scan_sources(src_dirs: list[str], prefixes: set[str]) -> dict[str, list[Location]]:
    """Scan source files for requirement tags; returns canonical id -> locations."""
    found: dict[str, list[Location]] = {}
    for src in src_dirs:
        base = Path(src)
        if not base.exists():
            continue
        for root, dirs, files in os.walk(base, topdown=True):
            dirs[:] = [d for d in dirs if d not in IGNORE_DIRS]
            for name in files:
                path = Path(root) / name
                if path.suffix.lower() not in SOURCE_EXTS:
                    continue
                try:
                    lines = path.read_text().splitlines()
                except OSError as exc:
                    print(f"Warning: could not read {path}: {exc}", file=sys.stderr)
                    continue
                for lineno, line in enumerate(lines, 1):
                    for pattern, trace_type in TAG_PATTERNS:
                        match = pattern.search(line)
                        if not match:
                            continue
                        for raw in ID_SPLIT.split(match.group(1)):
                            idm = ID_RE.match(raw.strip())
                            if not idm or idm.group("prefix").upper() not in prefixes:
                                continue
                            found.setdefault(canonical(raw), []).append(
                                Location(str(path), lineno, line.strip(), trace_type)
                            )
    return found


def merge(reqs: dict[str, Trace], found: dict[str, list[Location]]) -> dict[str, Trace]:
    """Attach scanned locations to requirements; unknown ids become orphan traces."""
    for cuid, locs in found.items():
        trace = reqs.get(cuid)
        if trace is None:
            trace = Trace(uid=cuid, display=cuid, known=False)
            reqs[cuid] = trace
        for loc in locs:
            target = trace.implementations if loc.trace_type == "implements" else trace.verifications
            target.append(loc)
    return reqs


def _report_dict(reqs: dict[str, Trace]) -> dict:
    known = sorted((t for t in reqs.values() if t.known), key=lambda t: t.uid)
    orphans = sorted((t for t in reqs.values() if not t.known), key=lambda t: t.uid)
    return {
        "summary": {
            "requirements": len(known),
            "implemented": sum(1 for t in known if t.implementations),
            "verified": sum(1 for t in known if t.verifications),
            "uncovered": sum(1 for t in known if not t.implementations),
            "orphan_tags": len(orphans),
        },
        "requirements": [
            {
                "uid": t.display,
                "text": t.text,
                "implementations": [f"{l.file}:{l.line}" for l in t.implementations],
                "verifications": [f"{l.file}:{l.line}" for l in t.verifications],
            }
            for t in known
        ],
        "orphans": [
            {"uid": t.display, "references": [f"{l.file}:{l.line}" for l in t.implementations + t.verifications]}
            for t in orphans
        ],
    }


def render_markdown(data: dict) -> str:
    s = data["summary"]
    out = [
        "# Traceability Report",
        "",
        "Generated by `scripts/traceability.py` (`make traceability-report`).",
        "",
        "## Summary",
        "",
        f"- Requirements: {s['requirements']}",
        f"- Implemented: {s['implemented']}",
        f"- Verified: {s['verified']}",
        f"- Uncovered (no implementation): {s['uncovered']}",
        f"- Orphan tags (reference an unknown requirement): {s['orphan_tags']}",
        "",
    ]
    if data["requirements"]:
        out += ["## Requirements", "", "| UID | Implemented | Verified |", "| --- | --- | --- |"]
        for r in data["requirements"]:
            impl = "<br>".join(r["implementations"]) or "—"
            verif = "<br>".join(r["verifications"]) or "—"
            out.append(f"| {r['uid']} | {impl} | {verif} |")
        out.append("")
    if data["orphans"]:
        out += ["## Orphan tags (references to unknown requirements)", ""]
        for o in data["orphans"]:
            out.append(f"- `{o['uid']}` referenced at {', '.join(o['references'])}")
        out.append("")
    if not data["requirements"] and not data["orphans"]:
        out += ["_No requirement items or code tags yet._", ""]
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate a requirements traceability report.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument("--output", type=Path, help="write to FILE instead of stdout")
    parser.add_argument("--src", action="append", help="source dir to scan (repeatable)")
    parser.add_argument("--requirements-root", default=DEFAULT_REQUIREMENTS_ROOT)
    parser.add_argument("--check", action="store_true", help="exit 1 if orphan tags exist")
    args = parser.parse_args()

    reqs = load_requirements(Path(args.requirements_root))
    prefixes = valid_prefixes(reqs)
    found = scan_sources(args.src or DEFAULT_SRC_DIRS, prefixes)
    reqs = merge(reqs, found)
    data = _report_dict(reqs)

    rendered = json.dumps(data, indent=2) if args.format == "json" else render_markdown(data)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered + "\n")
        print(f"Wrote {args.output}", file=sys.stderr)
    else:
        print(rendered)

    if args.check and data["summary"]["orphan_tags"]:
        print(f"FAIL: {data['summary']['orphan_tags']} orphan tag(s) reference unknown requirements.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
