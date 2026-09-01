---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e3cf7aed9a57",
  "title": "Headless CLI bulk-migrate (future)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: iQxRLtpWED4-OtG1b_KvIa4h1lfWLAPIJvMUF5TOXcM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "STOR-schemaCliMigrate"
  }
}
---
A headless command-line invocation of the bulk-migrate action,
suitable for continuous-integration or scripted upgrades across
many Projects, is deferred. The initial bulk-migrate capability
lives only in the UI. This deferral rides with the wider
CLI / headless deferral captured in REPORT-cliExport and will be
revisited when CLI support becomes a concrete need.
