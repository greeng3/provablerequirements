---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e152381d9eb1",
  "title": "MCP read-write operations (future)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: IWEsNis_D6b9YX-Gk-bfrhFRy4X2gBdzHEvg2CCOqf8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.18",
  "legacy": {
    "doorstopUid": "LLM-mcpReadWrite"
  }
}
---
Write-capable MCP operations — creating artifacts, editing
artifact content, creating links, submitting review actions —
are deferred until the read-only surface (per LLM-mcpServer,
LLM-mcpTools, LLM-mcpResources, and LLM-mcpPrompts) has seen
real-world use and the review workflow has been demonstrated
to contain AI-authored drafts safely. When enabled, AI-
created artifacts and edits shall land in an unapproved
review state by default, relying on the existing REVIEW-*
workflow to gate human approval before the content is
considered authoritative. This deferral is scoped narrowly:
the read-only MCP surface ships as a first-class feature, not
as a deferred future item.
