---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0bd1a37cfce",
  "title": "Prefix collision with existing Collections",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: pHwAh2d-tGxLDPfQB5VpFAC7_OSq4V4v2nK1m0vyT8Y="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopPrefixCollision"
  }
}
---
If a doorstop document's prefix matches the prefix of an
existing ReqForge Collection in the target Project, the importer
shall halt before writing any files and report the collision in
the import report. Neither the existing Collection nor the
doorstop source shall be modified. The user must resolve the
collision — typically by renaming one of the prefixes or by
removing the conflicting ReqForge Collection — before re-running
the import.
