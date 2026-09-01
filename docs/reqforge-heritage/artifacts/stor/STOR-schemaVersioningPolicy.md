---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4201940da9c",
  "title": "Schema versioning policy",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: EU-8KzCAusuyU_ECBbNkjuIdXCVzBmvg3YpGghWeLAw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.11",
  "legacy": {
    "doorstopUid": "STOR-schemaVersioningPolicy"
  }
}
---
Before ReqForge's 1.0 release, schema versions may be bumped
liberally as the design stabilises; each bump still requires a
registered forward-migration function, but breaking changes are
expected. From 1.0 onward, any schema bump shall be treated as a
breaking change under a semantic-versioning posture for the tool:
schema bumps ride with a ReqForge major-version bump, and
migration paths are maintained for all prior schema versions.
Downgrading schemas (for example, to share files with users on an
older ReqForge) is explicitly out of scope; teams collaborating
on a System are expected to run the same ReqForge version.
