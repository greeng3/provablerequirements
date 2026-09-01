---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e74a70b4330e",
  "title": "Full-text search and filtering",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: VmzYGQd5VfdWXEi8nITpd5k6uN69PBmtmp6m5tP503U="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "UX-search"
  }
}
---
ReqForge shall provide full-text search over artifact content,
spanning all mounted projects within a System. The indexed
corpus covers artifact title, artifact short name (the name
portion of the UID, for example "gitNative" in
"STOR-gitNative"), body, description, and tags.
Search shall be combinable with structured filters, including
at minimum artifact type, review state, link presence,
active/inactive state, Project, and Collection. The back-end
indexer shall be Tantivy (pure Rust, Lucene-style, MIT-
licensed) with the index held in memory for the lifetime of
the container, rebuilt on startup alongside the UUID index
(per TRACE-uuidIndex). Query syntax inherits Tantivy's native
support for phrase queries, field-scoped queries, and boolean
operators (AND, OR, NOT). Moving to an on-disk Tantivy index
is a later option if memory pressure warrants it; the initial
in-memory design matches the UUID-index posture.
