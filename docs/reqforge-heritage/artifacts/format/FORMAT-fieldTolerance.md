---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df714a0ce349",
  "title": "Field-presence and unknown-field handling",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: zbugP6XSjIlJDP8vNyY_ECSo8opE92detWFtoo2oKlY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.19",
  "legacy": {
    "doorstopUid": "FORMAT-fieldTolerance"
  }
}
---
When loading an artifact or configuration file:
  - If the file is parseable as JSON but is missing a field
    the current schema marks as required, the artifact fails
    to load and is surfaced in the UI with a clear
    per-artifact error message; the containing Project and
    other artifacts continue to load so the System remains
    usable.
  - Unknown fields (fields present in the file that the
    current ReqForge schema does not recognise) are preserved
    verbatim in an in-memory overflow bucket and written back
    unchanged on the next save of that artifact or
    configuration file. This keeps files written by newer or
    forked ReqForge versions round-trippable without data
    loss, complementing the explicit legacy field on
    artifacts (per ART-legacyField) which serves the similar
    purpose for importer-preserved content.
