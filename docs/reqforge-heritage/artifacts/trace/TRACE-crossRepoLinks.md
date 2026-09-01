---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4fd0a287067",
  "title": "Cross-repository links",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: LjDY_z8nwiaACBkWUU4DrMQGUD947dcoMYzvn_IQ2Lk="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "TRACE-crossRepoLinks"
  }
}
---
Links shall be permitted to span projects and therefore git
repositories. The link payload shall be storable in any one of the
involved repositories (typically the source end). Cross-repository
resolution shall use the UUID index built across all currently
mounted projects.
