---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e680a2f44b1d",
  "title": "Graph canvas link authoring",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: llNRoLbDYymCSseUwo9VKY5ae7ktCufnqoT2wy7yPWo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "UX-linkCreationGraph"
  }
}
---
ReqForge shall provide a graph canvas view for authoring and
exploring links. On this canvas, users shall be able to connect
artifacts visually via drag-to-link, pick link types, and inspect
surrounding neighbourhoods of the traceability graph. The
implementation shall use React Flow (MIT, actively maintained)
as the underlying graph library. The default layout is
force-directed; a hierarchical layout option is offered
automatically when the current view is scoped to acyclic link
types (derives-from, supersedes). The view imposes a soft cap
of approximately 500 nodes; beyond that, the UI prompts the
user to apply filters (Project, Collection, link type, or tag)
to narrow the set before rendering. Scaling strategies for
graphs beyond that cap (hierarchical overview, on-demand
expansion, dependency- or status-based filtering) are deferred
until encountered in practice.
