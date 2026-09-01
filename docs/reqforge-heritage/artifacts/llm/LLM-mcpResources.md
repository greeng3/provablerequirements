---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e16aba795280",
  "title": "MCP resources exposed",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: ls-v4jFL-OuxGK8cYB0-xE8ZciB5iHXFmF1dE2lH-gE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.16",
  "legacy": {
    "doorstopUid": "LLM-mcpResources"
  }
}
---
The MCP server shall expose each ReqForge artifact as a
readable MCP resource identified by a stable URI of the form
reqforge://artifact/<uuid>. Resource content varies by
artifact shape:
  - Content-hosted artifacts: the rendered Markdown body,
    with relevant metadata (title, tags, review state)
    included as a compact header the agent can parse.
  - Blob artifacts: metadata and a reference pointer to the
    paired binary; the blob bytes themselves are retrievable
    via a separate resource URI (for example,
    reqforge://blob/<uuid>).
  - URL-reference artifacts: metadata and the stored URL as
    the resource body.
Agents may subscribe to resource-list updates so newly-
created or newly-modified artifacts appear in the agent's
context without manual refresh.
