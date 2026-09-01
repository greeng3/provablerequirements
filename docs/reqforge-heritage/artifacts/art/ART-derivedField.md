---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b5e6858b2a7a",
  "title": "Derived-from-external-source flag",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: -Zg0Ht_JptsFuwN1YrkcW7RscfhxyKQuOhVzjkLLPts="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "ART-derivedField"
  }
}
---
Every artifact shall carry a derived field (boolean, default
false) indicating whether the artifact is derived from an
external source — a standard, regulation, contract, or similar —
rather than authored internally. The flag is informational;
ReqForge shall surface it in the artifact view so reviewers can
distinguish externally-sourced content from internally-authored
content. The derived flag does not by itself affect traceability
reports or review workflow.
