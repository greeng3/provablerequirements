---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-ddc8316a0b58",
  "title": "UID/GID match repository ownership",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:08:02.594607629Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-deca5fe284c6",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-singleUserLocalhost"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: St5dUxfyKInmqx8RJX0bBNWsrHXyB_gGPYa0iD1Oh7Y="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "DEPLOY-chownFromDotGit"
  }
}
---
After writing a file to a bind-mounted repository, ReqForge shall
ensure the resulting file's UID and GID match those of the
repository's .git entry. Where .git is a regular file pointing to a
worktree's real git directory, ReqForge shall follow that pointer to
determine the owner. This prevents files from being created as root
and causing later permission problems for the user.
