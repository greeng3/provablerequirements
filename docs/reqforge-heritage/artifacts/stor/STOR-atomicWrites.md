---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e366f5de17ac",
  "title": "Atomic writes via temp-file-then-rename",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: CrwEmStt2AjQNfRdg1htyLXsgm2NrtB-EmI7RoxfcOc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "STOR-atomicWrites"
  }
}
---
Every ReqForge-managed file write shall be atomic: ReqForge
writes the new content to a sibling temporary file in the same
directory as the target, fsyncs the temp file, and then renames
it into place over the target. This guarantees that a reader
(including another ReqForge session or any other process) never
observes a partially-written file regardless of when ReqForge
crashes, is killed, or loses power. Crash safety is a
first-class requirement rather than a nice-to-have: ReqForge is
expected to run in home-lab, dev-container, and small-team
infrastructure where flaky restarts, OOM kills, and power loss
are routine events.
