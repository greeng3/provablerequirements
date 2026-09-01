---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df4b9288644a",
  "title": "Collections root path",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: xZPFyZtpqaKsKenaRY1JF9vcLvHx2VVNmVn0icsrlrw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "FORMAT-collectionsRootPath"
  }
}
---
By default, a Project's Collections shall live as subdirectories
under an "artifacts" directory at the repository root; each such
subdirectory is a Collection carrying its own .collection.json
and artifact files (for example,
artifacts/requirements/REQ-gitNative.md). A Project may override
this root by declaring an optional artifactsPath field in its
reqforge.json, interpreted relative to the repository root. The
override is intended to be rare — the default placement is
strongly preferred for discoverability and for consistent tooling
across Projects. The name "artifacts" is chosen because
ReqForge manages more than just requirements (design documents,
use cases, diagrams, roadmaps, uploaded files, URL references),
and "artifacts" captures the full scope without tool-branding
the user's repository.
