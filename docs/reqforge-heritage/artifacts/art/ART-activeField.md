---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b59aa463261c",
  "title": "Active/inactive artifact lifecycle flag",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: WBk_U5x1bMEHImP5uYxBq913LtloEmz5FwGqbRu24R4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.14",
  "legacy": {
    "doorstopUid": "ART-activeField"
  }
}
---
Every artifact shall carry an active field (boolean, default
true). Inactive artifacts remain in the repository and continue
to participate in link resolution — other artifacts may still
link to or from them — but they shall be excluded by default
from uncovered-artifact counts in coverage and code-traceability
reports and from the System-wide review queue. A "show all /
include inactive" toggle in the UI and an equivalent report-
scope option shall reveal them when needed. This supports
deprecation of artifacts without deletion and without losing
historical traceability.
