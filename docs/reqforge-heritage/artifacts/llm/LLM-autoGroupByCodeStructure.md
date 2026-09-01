---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0ff88021d98",
  "title": "Auto-grouping by code structure (future)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: rz3ehVA-0pstULBEKnlFC2QclA3L_51c3KwsTlZlDy4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "LLM-autoGroupByCodeStructure"
  }
}
---
ReqForge shall eventually propose Collection groupings derived
from repository structure — for example, one Collection per
Rust workspace crate, per npm monorepo package, per Docker
Compose service, per Go module, per Python package, per
standalone shell script directory, and similar groupings for
other languages. The grouping detector shall cover every
language configured in the scanner language registry (per
TRACE-codeLanguageRegistry), using each language's idiomatic
structural markers (Cargo.toml, package.json, pyproject.toml,
go.mod, Dockerfile, and similar). The user reviews and
accepts, modifies, or rejects each proposed grouping. The
feature is deferred.
