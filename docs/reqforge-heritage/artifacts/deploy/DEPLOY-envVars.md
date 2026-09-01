---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de2aaca04f50",
  "title": "Container environment variables",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T23:07:51.954674096Z",
  "links": [
    {
      "targetUuid": "019df9d6-cbf6-7ac2-8a79-dee8d07057ee",
      "type": "related-to",
      "hint": {
        "projectSlug": "reqforge",
        "collectionPrefix": "DEPLOY",
        "artifactName": "DEPLOY-systemAboveProject"
      }
    }
  ],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: ZMrn-UEqx691yPs7Bh1hFnOf-rRtp6IuijF6ed4c4Fc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.14",
  "legacy": {
    "doorstopUid": "DEPLOY-envVars"
  }
}
---
ReqForge shall honour the following environment variables when
run as a container:
  - REQFORGE_MOUNT_PREFIX (default /repos): the in-container path
    prefix scanned for bind-mounted repositories, per
    DEPLOY-mountConvention.
  - REQFORGE_SYSTEM_CONFIG (no default): the full path to the
    System configuration file inside the container. When unset,
    ReqForge runs in unnamed-system mode without cross-Project
    features that depend on the System config.
  - REQFORGE_PORT (default 36743): the TCP port on which the web
    UI is served, per DEPLOY-defaultPort.
  - REQFORGE_LOG_LEVEL (default info): logging verbosity for the
    ReqForge back-end.
  - REQFORGE_UID and REQFORGE_GID: optional overrides for the
    file-ownership matching that normally derives from each
    repository's .git entry (per DEPLOY-chownFromDotGit). Useful
    on hosts where the .git owner is not the intended owner of
    ReqForge-written files.
