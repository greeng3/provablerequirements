---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1f473e3dc75",
  "title": "LLM-assisted rename workflow",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 2uShaLjsGOYEXAv-T83gdnPkDktzyIIIigD-32FwOHQ="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "LLM-renameWorkflow"
  }
}
---
When at least one LLM provider is configured and currently
healthy, ReqForge shall offer a "Suggest name with LLM" action
on each artifact and as a bulk action on Collection views. The
action sends the artifact's title, description, tags, and body
content to the active provider and parses the response as a
proposed new artifact name. The UI displays current and
proposed names side-by-side, per artifact; the user approves
or rejects each suggestion individually, or accepts all
currently-visible suggestions in a single batch action. Rename
mechanics reuse ART-moveRename: UUID is stable, incoming links
persist, human hints refresh lazily.
