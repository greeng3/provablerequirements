---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de0c2e1e3c70",
  "title": "Developer workspace directory convention",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: rp129g6pPi53B9dVZUYcOCZa6MPUNOjzwCCcHEx7hN4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.17",
  "legacy": {
    "doorstopUid": "DEPLOY-devWorkspace"
  }
}
---
The ReqForge source repository shall contain a
.reqforge-workspace directory at its root for developer-time
configuration and test fixtures. The directory shall be
gitignored except for committed example files that serve as
templates:
  - example-system.json: seed for a dev's local System config.
  - example-docker-compose.yml: reference launch file for
    exercising the production container image from the dev
    environment.
A contributor working in the VS Code devcontainer copies the
example files to uncommitted counterparts (system.json,
docker-compose.yml), optionally places test Project
repositories under .reqforge-workspace/test-repos, and runs
ReqForge natively with REQFORGE_MOUNT_PREFIX pointing at the
test-repos directory. The hidden name mirrors the operator
convention (per DEPLOY-operatorWorkspace) so the semantics are
consistent between dev and production.
