---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e09b441016f5",
  "title": "Doorstop item to artifact mapping",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: ady3DNK0ic1OG6Vq23We3PqMH5SZBVX7mtvFbCrFY88="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopItemMapping"
  }
}
---
Each doorstop item shall be translated into a content-hosted
ReqForge artifact — a Markdown file with JSON frontmatter in the
Collection's directory. Field-by-field translation:
  - header -> title
  - text   -> the Markdown body of the file (the prose after the
              frontmatter's closing ---)
  - active -> active
  - derived -> derived
  - level  -> outlineLevel (as a string, for example "1.2.3")
  - normative=true  -> no special handling
  - normative=false -> tags includes "non-normative"
  - links  -> typed links translated per
              INTEROP-doorstopLinkTranslation
  - ref    -> per INTEROP-doorstopRefHandling
  - reviewed -> per INTEROP-doorstopReviewedHash
  - any other extension fields (custom data, tags, verification,
    and similar) -> preserved verbatim in the legacy object.
Each imported artifact receives a newly generated UUID. The
createdAt and modifiedAt timestamps are set to the import time.
