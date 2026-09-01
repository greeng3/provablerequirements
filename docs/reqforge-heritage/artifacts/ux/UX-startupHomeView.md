---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e77301868c99",
  "title": "Startup home view listing mounted repositories",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: CEH9rqlpBvpHQiK8l-3rV_nTMChSSbP-fQu3oGtmMQg="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.18",
  "legacy": {
    "doorstopUid": "UX-startupHomeView"
  }
}
---
On startup, the ReqForge UI shall present a System Home view
listing each bind-mounted repository found under the mount-
prefix convention (per DEPLOY-mountConvention), annotated with
the mount's validity state — Project, Needs-init, No-git, or
Read-only — as classified per DEPLOY-mountValidityStates. The
empty state, where no repositories are mounted, shall show
explicit guidance on adding bind mounts to the user's compose
or docker run configuration rather than a generic empty-list
message.
