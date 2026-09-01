---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e73ef10bbab0",
  "title": "Scaling targets (soft)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: pv2aEmbhrSsBavTzyXdnNUExEcvTz3QghMhJyJEw3DE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.31",
  "legacy": {
    "doorstopUid": "UX-scalingTargets"
  }
}
---
ReqForge's initial version shall target a small-team / home-
lab scale:
  - Up to approximately 10 mounted Projects per System.
  - Up to approximately 5,000 total artifacts across all
    mounted Projects.
Within this target envelope, ReqForge honours the performance
targets of UX-performanceTargets. Beyond it, ReqForge should
continue to function but performance is best-effort; indexer,
UI virtualisation, and startup patterns are revisited when
real workloads exceed the target. The target is a design
point, not a hard limit: users with larger workloads are not
rejected but are not a tuning focus for the initial version.
