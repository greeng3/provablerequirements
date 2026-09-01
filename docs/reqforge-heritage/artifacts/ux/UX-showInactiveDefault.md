---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e759d40733c0",
  "title": "Default visibility of inactive artifacts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: vLxR70PdwnIohACX5XB09hQF2ItDg1AvSPOlsYCIXKc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.33",
  "legacy": {
    "doorstopUid": "UX-showInactiveDefault"
  }
}
---
Every ReqForge view or report that supports a "show all /
include inactive" toggle shall default the toggle to off,
hiding inactive artifacts (per ART-activeField) unless the
user explicitly opts in. This applies uniformly across search
results, Collection browsing, the System-wide review queue,
coverage and code-traceability reports, and any similar list
surface. The toggle's state is per-view and per-session; it
does not persist across container restarts.
