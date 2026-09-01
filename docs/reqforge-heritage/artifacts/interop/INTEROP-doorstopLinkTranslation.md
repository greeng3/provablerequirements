---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0a4dd62bb58",
  "title": "Doorstop link translation",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: q8hzGfgYWfRCUlaOz7acgbStNqr-C3W3GlIYfgbPhrc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopLinkTranslation"
  }
}
---
Each UID in a doorstop item's links array shall be translated
into a typed link entry on the imported artifact, with type
derives-from — the closest semantic match to doorstop's
untyped parent-child relationship. Each link entry shall record
the target artifact's newly assigned ReqForge UUID, the
derives-from type, and the human hint comprising the source
Project's slug, the target Collection's prefix, and the target
artifact's post-import name (per TRACE-linkIdentityFormat).
If a doorstop link targets a UID that cannot be resolved to an
imported artifact (for example, because the target document was
not in scope for the import), the link shall be surfaced in the
import report as an unresolved link but the reference is still
written with its hint populated, letting
TRACE-unresolvedLinks handle it thereafter.
