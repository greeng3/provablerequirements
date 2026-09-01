---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dff635d34c08",
  "title": "Qualified .reqforge.json extension for pure-metadata files",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: TD1neeR-6di1lRhyEB-D5LRb2XKAf6UIRZ_bjYRrovE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "FORMAT-qualifiedExtension"
  }
}
---
Pure-metadata files authored by ReqForge (blob sidecars and URL
artifact files) shall use the compound extension .reqforge.json
rather than plain .json. The qualified extension makes ReqForge
files visually identifiable in a repository tree and easy to target
with glob patterns in external tooling. Configuration files
(reqforge.json, .collection.json, and the System config) retain
their specific filenames rather than adopting the .reqforge.json
extension, because those files are recognised by name.
