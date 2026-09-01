---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b62cc5bfe17a",
  "title": "Legacy container for unmapped import fields",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: IqEbrU7KNNJxxemDnvnwd9kliY4hMGm13hav9GYuEKU="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.18",
  "legacy": {
    "doorstopUid": "ART-legacyField"
  }
}
---
Every artifact may carry an optional legacy field (object)
holding arbitrary key-value data ReqForge does not interpret.
The legacy field shall be preserved verbatim across reads and
writes and is primarily used by importers — such as the
doorstop importer — to stash source-system fields that ReqForge
does not translate into first-class fields. Users may
subsequently migrate legacy contents into tags, description
text, or explicit links via a one-time script if they choose,
or delete the legacy block when no longer needed.
