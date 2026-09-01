---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e69d884121ed",
  "title": "Matrix link view",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: TKoY4YoS5_UfzQVXsoTsjIsVw9rfNME1TaiYPBxarFU="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "UX-linkCreationMatrix"
  }
}
---
ReqForge shall provide a matrix view of links, typically with one
axis listing source-side artifacts and the other listing
target-side artifacts, suitable for coverage and gap-analysis
work. Cells allow adding or removing links. The implementation
shall use TanStack Virtual (or an equivalent React virtualisation
library, MIT-licensed) to render only visible rows and columns,
keeping large matrices responsive. Axis filters narrow each axis
by artifact type, Collection, tag, or review state before the
matrix is rendered. The view imposes a soft cap of approximately
500 items per axis; beyond that, the UI requires additional
filtering before rendering. Scaling strategies beyond that cap
(chunked views, focus-on-one-artifact fan-out views, or similar)
are deferred until encountered in practice.
