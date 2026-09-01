---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e17486ee12b8",
  "title": "MCP server for AI agent access",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: uASzPEjVqvFqA_JRvXCR7ahFNPLsj0jOWKVh2hzX_-E="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.14",
  "legacy": {
    "doorstopUid": "LLM-mcpServer"
  }
}
---
ReqForge shall expose a Model Context Protocol (MCP) server as
the primary surface for AI coding agents (Claude Code, Claude
Desktop, Cursor, GitHub Copilot, Zed, and similar) to query
and consume its artifacts, traceability graph, and reports.
The MCP server is a thin adapter layered on the REST API (per
TECH-restApi); it does not replace that API but translates
ReqForge's operations into MCP's tools/resources/prompts call
shape. Initial deployment is localhost-only, consistent with
DEPLOY-singleUserLocalhost; no authentication is required
because the agent runs on the same host as ReqForge. The
server exposes three MCP capability categories — tools (per
LLM-mcpTools), resources (per LLM-mcpResources), and prompts
(per LLM-mcpPrompts). Read/write behaviour in the initial
version is read-only; read-write access is deferred (per
LLM-mcpReadWrite). The MCP surface is a first-class feature,
not a deferred future item — it substantially magnifies the
value of ReqForge's requirements to agent-assisted coding
workflows.
