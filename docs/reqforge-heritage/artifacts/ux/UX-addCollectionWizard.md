---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e5b8ba94812e",
  "title": "Create-Collection wizard for existing Projects",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: aDDRcGPI9i8eugt7StIgBma3I6DPdawhZW5S82sOcPM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.32",
  "legacy": {
    "doorstopUid": "UX-addCollectionWizard"
  }
}
---
The UI shall provide a "Create Collection" action on any
existing Project's page that opens the same Collection-
creation wizard used during post-init (per
UX-postInitChoice). The wizard requests prefix, name, an
optional description, and the expectsCodeTrace toggle. On
confirmation a new Collection directory is created under the
Project's Collections root (per FORMAT-collectionsRootPath),
its .collection.json is written, and the new Collection is
selected in the UI.
