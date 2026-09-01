---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e54a0d567835",
  "title": "One-sided link storage",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: geZgqKyyv-TpuiXTTxQPcerrMQBDSLHkKhHOVRjuSvY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "TRACE-oneSidedStorage"
  }
}
---
A link shall be stored once, in the metadata of the source-side
artifact. The reverse view of the link (the target's perspective)
shall be derived at query time from the UUID index rather than
stored redundantly in the target's metadata. This keeps a single
source of truth, halves the write work of creating or deleting a
link, and eliminates the risk of divergent bi-directional records.
