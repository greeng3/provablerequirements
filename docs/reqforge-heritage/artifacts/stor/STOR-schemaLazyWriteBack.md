---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e3fc002e27de",
  "title": "Lazy write-back of migrated files",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 8xvAnDFgToyY6UdO-pO-NTlODzUZ6QXi5_gK1Z7iLPk="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.8",
  "legacy": {
    "doorstopUid": "STOR-schemaLazyWriteBack"
  }
}
---
ReqForge shall not rewrite files on disk purely because their
schemaVersion is below the current known version. A file's
on-disk schemaVersion shall be bumped only when the user edits
the file through ReqForge, at which point the file is rewritten
at the current schema version as a side effect of the edit.
Because migrations are deterministic, a file read many times
before it is edited is re-migrated to the same in-memory form on
each read; there is no risk of compounding drift. Because the
on-disk original is untouched until an explicit edit, a bug in a
migration function does not corrupt files that were only read.
