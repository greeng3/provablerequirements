---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de8705a9bb4c",
  "title": "No git write operations; read-only gitoxide access permitted",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 3f9M8mAB7B5cTWwWbiyj4g3yJlD7xBr6Rd1gUQyNmdo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "DEPLOY-noGitOps"
  }
}
---
ReqForge shall not perform git write operations. Commits,
branches, merges, pushes, pulls, tag creation, and conflict
resolution shall remain the user's responsibility, performed
through their normal git client. This deliberately removes an
entire class of commit-authorship, branching, and merge-
conflict concerns from ReqForge's scope.
Read-only access to the git object store is permitted — via a
pure-Rust git library such as gitoxide — where ReqForge
features genuinely require historical version access (notably
UX-diffView's comparison against HEAD and prior commits, and
the "since last review" comparison in UX-reviewPane). Shelling
out to the git CLI is not used.
Read-only access shall not extend to exposing general
git-client functionality inside ReqForge's UI (branch lists,
commit graphs, staging areas, merge-conflict resolution
workflows, and similar). Those remain the user's git client's
responsibility; ReqForge is not a git UI.
