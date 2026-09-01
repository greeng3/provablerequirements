---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e43018751568",
  "title": "Minimal observability surface",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: pu0b_pXMia157EuRbhXR6O12gmekVAi3dguk7qyZSMY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "TECH-observability"
  }
}
---
ReqForge shall expose a small observability surface suitable
for single-operator deployment:
  - A GET /healthz endpoint returning 200 OK when the process
    is running. Intended for container liveness probes.
  - A GET /readyz endpoint returning 200 OK when project
    discovery has completed, the UUID index has been built,
    and the search index has been built. Returns 503 during
    startup or rebuild. Intended for container readiness
    probes.
  - An optional GET /metrics endpoint in Prometheus text
    format exposing a handful of counters and gauges — HTTP
    request count per route, search query count, indexer
    status, polling-watch event count, current project and
    artifact counts. The endpoint is disabled by default and
    enabled via an environment variable.
  - Logs written to stdout as JSON lines, honouring the
    REQFORGE_LOG_LEVEL environment variable per DEPLOY-envVars.
More elaborate observability (tracing, OpenTelemetry,
distributed metrics) is not committed for the initial version.
