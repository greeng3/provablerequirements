---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-ddd20d05e4f3",
  "title": "Docker Compose as the primary deployment path",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: LXTgSigYUyp-ysUDf5FFpmOnraBtTQzsYR3Zyf-eh9w="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "DEPLOY-composeDeployment"
  }
}
---
ReqForge's documentation shall treat docker-compose (or an equivalent
orchestration file) as the primary deployment path for multi-
repository setups. Hand-rolled docker run invocations with many bind-
mount flags become unwieldy and error-prone at realistic System
sizes. The documentation shall include a canonical
docker-compose.yml example illustrating the mount-prefix
convention, the System configuration bind, and the relevant
environment variables (per DEPLOY-envVars).
