---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b5c75582d9ec",
  "title": "Constraints as regular artifacts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: MpnX8TLE9PkGyQKg38QM7htfDOI6bAtzG1QSdkzuUg4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.19",
  "legacy": {
    "doorstopUid": "ART-constraintConvention"
  }
}
---
ReqForge shall not introduce a first-class "constraint" concept
distinct from regular requirements. Constraints (technology,
business, regulatory, environmental, standards, physical, and
similar) are represented as ordinary artifacts — typically
grouped in a dedicated Collection (for example, prefix CON)
and/or tagged "constraint" when they need to be surfaced across
multiple Collections. Typed links, particularly derives-from,
express the relationship between a design decision and the
constraint that motivated it. This keeps the artifact model
simple and avoids proliferating feature-specific branches in
filter, search, and report code.
