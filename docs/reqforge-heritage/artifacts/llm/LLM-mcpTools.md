---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e18b50935e96",
  "title": "MCP tools exposed",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 5LvVJsa5rCZr9QO85OiWNsdF0JgSAgpWLwP4w0XaHhs="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "LLM-mcpTools"
  }
}
---
The MCP server shall expose the following tool categories at
minimum:
  - Artifact reads: fetch an artifact by UUID or by
    (projectSlug, collectionPrefix, artifactName); list
    artifacts in a Collection or Project, with optional
    filters on tags, review state, and active/inactive state.
  - Search: full-text search with the same filters the UI
    supports (per UX-search), returning matching artifact
    identifiers and relevance-ranked snippets.
  - Link / graph walks: return an artifact's outgoing links,
    incoming links (derived via the UUID index per
    TRACE-uuidIndex and one-sided storage per
    TRACE-oneSidedStorage), or the multi-hop neighbourhood of
    an artifact limited by depth and link-type filters.
  - Report generation: invoke any report in the REPORT
    Collection, with the same scope selector the UI supports
    (per REPORT-scopeSelector), returning the report content
    in JSON.
  - Review-log reads: fetch the review log for an artifact,
    including blocking TODOs and the since-last-approval
    summary (per UX-reviewPane).
All tools in the initial version are read-only. Write-capable
tools are deferred per LLM-mcpReadWrite.
