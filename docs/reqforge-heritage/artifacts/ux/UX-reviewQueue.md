---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e7259d69a8cb",
  "title": "System-wide review queue",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: N_ZH5AYWHoSEc-HlY10nXNVoxfr-I3S0V0I2uJUSGhA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "UX-reviewQueue"
  }
}
---
ReqForge shall provide a System-wide review queue with two
distinct sections:
  - Awaiting review: artifacts whose current state has no
    approved review-log entry covering it. Default ordering is
    oldest-modification-first so that nothing languishes; the
    secondary sort is by Project then Collection. The user can
    switch to newest-first, group-by-Collection, or filter.
  - Unresolved blocking TODOs: artifacts that have one or more
    blocking TODOs still open from a prior rejected review.
    These items are waiting on the author, not on a reviewer,
    and appear in their own section below the review-ready
    list.
The queue shall be filterable by Project, Collection, artifact
type, review state, tag, and reviewer identity in either
section.
