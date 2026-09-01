---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b5d8df07a118",
  "title": "Deletion semantics",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: IJPyRYzl4mGBN82p8uYPj028h64sHe7B3AKvHyaBl3U="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.21",
  "legacy": {
    "doorstopUid": "ART-deletionSemantics"
  }
}
---
Deletion operations shall behave as follows:
  - Artifact deletion: the UI warns the user of any incoming
    links from other artifacts before deletion, requiring
    explicit confirmation. After confirmation, the artifact
    file is removed; for a blob artifact, the paired binary
    file and the sidecar are removed atomically in the same
    operation. Incoming links on source artifacts are left
    intact and surface thereafter as unresolved (per
    TRACE-unresolvedLinks). ReqForge shall not auto-rewrite
    source-side artifacts to scrub the now-dangling link.
  - Collection deletion: permitted only when the Collection
    is empty. If artifacts remain, ReqForge shall refuse the
    deletion and prompt the user to move or delete them
    individually first. Deletion removes the Collection
    directory and its .collection.json.
  - Project deletion: the user removes the Project's
    reqforge.json (directly or through a "remove this
    Project" UI action that rewrites nothing else) and
    unmounts the repository. ReqForge does not cascade-delete
    any of the repository's artifact files; the user is
    responsible for those. ReqForge drops the Project's
    in-memory index entries on the next scan.
