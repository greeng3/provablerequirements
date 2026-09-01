---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e6f42e8daaaa",
  "title": "Post-initialisation choice",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: mhUgOVMC2EACDePx3b63z_GC9iik0cD4rwkagcoKvqM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.20",
  "legacy": {
    "doorstopUid": "UX-postInitChoice"
  }
}
---
After Project initialisation completes (per UX-initProjectWizard),
ReqForge shall present a three-way choice screen for populating
the new Project:
  - Create first artifact: proceeds to the artifact-creation UI
    (per UX-createArtifactUx) for a chosen shape.
  - Create sample content: pre-populates a small demonstration
    set of Collections and example artifacts (per
    UX-initSampleContent).
  - Import from doorstop: runs the doorstop importer to populate
    Collections and artifacts from existing doorstop content in
    the mount.
The "Import from doorstop" choice shall appear only when the
mount contains .doorstop.yml markers indicating existing doorstop
content; otherwise it is hidden.
