---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e04cd47a4ad4",
  "title": "Doorstop source discovery",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Q3itI_N55maMt99-ccaHWyRF5D332qyefyuUPlzt7Ks="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.2",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopDiscovery"
  }
}
---
The doorstop importer shall discover doorstop content by walking
the target repository's tree looking for .doorstop.yml marker
files. Each .doorstop.yml identifies a doorstop document whose
items are candidates for import. A .doorstop.yml that declares
no items or whose containing directory is otherwise empty shall
produce an empty Collection (warned in the import report) rather
than being skipped silently.
