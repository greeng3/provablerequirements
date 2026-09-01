---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf5-7133-a8e2-b69a26f5ddbb",
  "title": "Binary artifact update is replace-only",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Cgcit2cK16M43DNOEaGHrPKV18xO-cUdsqE8UigThRA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "ART-uploadReplaceOnly"
  }
}
---
ReqForge shall not provide an in-app editor for binary or complex
structured artifact formats (Microsoft Office documents, PDFs,
images, and similar). Updating such an artifact is accomplished by
uploading a new version, which replaces the on-disk file while
preserving the artifact's UUID, review log, and incoming and
outgoing links.
