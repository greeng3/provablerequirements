---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e05418f8431b",
  "title": "Doorstop document to Collection mapping",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: pKen4J44_6mLrdC7xNTvludQpt2IJJkXJHgPBgo5YDs="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopDocumentMapping"
  }
}
---
Each discovered doorstop document shall be translated into a new
ReqForge Collection under the target Project's Collections root
(artifacts/ by default, or the Project's artifactsPath override).
The Collection shall inherit the doorstop document's prefix as
its own prefix; its name is generated from the prefix and is
editable post-import. Doorstop document-level settings not
directly translated into first-class ReqForge fields — including
parent, sep, digits, and itemformat — shall be preserved verbatim
in the Collection's importNotes object (for example,
{"doorstopParent": "REQ", "doorstopSep": "-", "doorstopDigits":
3, "doorstopItemFormat": "yaml"}). The Collection's directory
name is a slugified form of the prefix by default.
