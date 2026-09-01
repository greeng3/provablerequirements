---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dfe4bc8fb579",
  "title": "Project configuration schema",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: r5xAR2MtvZFCiqA-4J9_60HhzXnGz2XJ4Q81mN0hqKs="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "FORMAT-projectConfigSchema"
  }
}
---
A Project's reqforge.json shall contain at minimum:
  - schemaVersion (integer): the project-config schema version.
  - slug (string): the Project's stable, unique identifier within
    its System.
  - name (string): a human-readable display name.
Optional fields include:
  - description (string): prose describing the Project.
  - artifactsPath (string): repository-relative path overriding
    the default "artifacts" Collections root (see
    FORMAT-collectionsRootPath).
  - scanPaths (array of strings): source paths for the code-trace
    scanner, interpreted relative to the repository root;
    defaults apply when omitted (see TRACE-codeScanConfig).
