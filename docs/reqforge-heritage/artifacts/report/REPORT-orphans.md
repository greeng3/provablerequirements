---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e295a10c7026",
  "title": "Orphans report",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: FGDFSdpJ4mUw-ACnOBNBcmdQ2CHrSX72l_fLS9qXQa0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "REPORT-orphans"
  }
}
---
ReqForge shall produce an orphans report covering two kinds of
orphan:
  - Link-graph orphans: artifacts with no incoming or outgoing
    traceability links. Typically stale artifacts or artifacts
    that have been miscategorised.
  - Filesystem orphans: mismatches in the on-disk pairing
    required for uploaded-blob artifacts (per FORMAT-blobSidecar).
    Specifically, a .reqforge.json sidecar whose blobPath
    target is missing, and a binary file in a blob-holding
    Collection directory without a companion .reqforge.json
    sidecar. For missing-blob cases, the user is prompted to
    restore the file or delete the sidecar; for orphaned-file
    cases, an "Adopt as artifact" action creates a sidecar via
    a short wizard. ReqForge shall never silently delete
    either kind of file.
