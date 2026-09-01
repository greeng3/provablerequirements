---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e06ac2d59cb5",
  "title": "Doorstop identifier normalisation",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: lam7EZMzJGmRmrYHVNrlcv0PLrH8xuQsqokScTJ5buw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.8",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopIdNormalization"
  }
}
---
The ReqForge artifact name for an imported item shall be derived
from the doorstop NANU — the portion of a doorstop UID following
its prefix and separator. The ReqForge UID is then
<prefix>-<name>, using the standard - separator. Rules:
  - Numeric padding is preserved: doorstop REQ001 becomes
    ReqForge REQ-001, not REQ-1.
  - If the doorstop NANU contains any - characters (possible when
    the doorstop document's sep is - and the NANU is multi-word,
    for example DES-rocket-nozzle), those - characters in the
    NANU shall be replaced with _ on import, yielding for example
    DES-rocket_nozzle.
  - The original doorstop UID shall be preserved verbatim in the
    imported artifact's legacy.doorstopUid field so the mapping
    from old name to new is recoverable.
Users may rename artifacts after import; post-import renaming is
outside the importer's scope.
