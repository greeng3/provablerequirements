---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de9fe6531a27",
  "title": "Operator workspace directory convention",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: B_jVinBVGzeCTrThZeyoH0Oh80wuDOxff104FtDotk8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.16",
  "legacy": {
    "doorstopUid": "DEPLOY-operatorWorkspace"
  }
}
---
A production operator shall maintain a workspace directory
separate from any managed Project repository, conventionally
located at ~/.reqforge-workspace on the host. The operator
workspace holds:
  - docker-compose.yml: the operator's launch configuration for
    ReqForge.
  - system.json: the System configuration file (per
    FORMAT-systemConfigSchema).
  - .env: environment variables supplying secrets (LLM API
    keys, and similar) referenced by the llm array and other
    configuration (per LLM-secretsViaEnv); this file is
    expected to be gitignored if the operator places their
    workspace under version control.
The hidden (.reqforge-workspace) form keeps the operator's
tool-specific configuration out of a casual "ls" listing
without concealing it from users who know where to look.
Managed Project repositories live wherever the operator
normally keeps them and are bind-mounted into the ReqForge
container independently.
