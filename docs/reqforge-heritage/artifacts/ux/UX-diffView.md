---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e60c9c009034",
  "title": "Diff view per artifact shape",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: BoPHFPxWApwzsn6w_HaMD3OkO1SDz-CHJ-DeP-B-tn8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.11",
  "legacy": {
    "doorstopUid": "UX-diffView"
  }
}
---
ReqForge shall provide a diff view for artifacts, with behaviour
depending on artifact shape:
  - Content-hosted artifacts: textual diff against HEAD and across
    recent commits.
  - Binary-blob artifacts: a metadata-level diff (size, hash,
    modified time) plus a side-by-side rendered preview using the
    artifact's normal viewer. No line-level text diff is produced for
    binary blobs.
  - URL-reference artifacts: diff of the URL string, accompanied by
    a note that the external content referenced by the URL is not
    under ReqForge's version control.
Historical versions are accessed by reading the git object store
via a pure-Rust git library (gitoxide or equivalent), not by
shelling out to the git CLI. Git write operations remain out of
scope per DEPLOY-noGitOps; the diff view is a read-only
consumer of git history.
