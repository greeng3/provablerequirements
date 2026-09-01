---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de3d4e198641",
  "title": "Makefile targets for standard operations",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: tV4DymHPwnP0Aqo8Ar85jvLqwJOlBXCpywwQvuCWz-8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.18",
  "legacy": {
    "doorstopUid": "DEPLOY-makefileTargets"
  }
}
---
The ReqForge repository shall expose a Makefile with targets
for every routine developer and operator operation, so that
users and contributors do not need to recall long command
invocations. The initial target set shall include at minimum:
  - make dev: run back-end and front-end dev servers together,
    using the configuration in .reqforge-workspace/.
  - make build: release build of the Rust back-end and
    production build of the React front-end.
  - make test: run all test suites (unit, integration, and any
    end-to-end flows).
  - make docker-build: build the production container image.
  - make docker-run: launch the production image locally via
    the reference docker-compose.yml for smoke testing.
  - make docker-publish: push the built image to the
    configured registry (opt-in via environment variables so
    that contributors without publish credentials are not
    blocked).
  - make fmt, make fmt-check, make lint: formatting and
    linting across all languages in use (existing targets from
    the imported Makefile continue to apply).
README recipes shall reference these targets rather than
document the underlying commands directly, so adding a new
convenience action requires only a Makefile change.
