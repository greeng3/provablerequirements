---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b678653ce0dd",
  "title": "Free-form tags for categorisation",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: EgkArseX2jvX4BENJKoXS4rmYQsdL-Tkh8QUQjU1BH0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.16",
  "legacy": {
    "doorstopUid": "ART-tagsField"
  }
}
---
Every artifact shall carry a tags field (array of strings,
default empty). Tags are free-form labels for cross-cutting
categorisation (for example, "phase-1", "high-priority",
"security", "non-normative"). Tags shall support search and
filter views in the UI and shall be usable as a grouping axis in
reports. Tags shall not establish traceability relationships;
ReqForge shall not auto-generate links from tag membership.
Explicit links via the link catalog remain the sole source of
traceability.
