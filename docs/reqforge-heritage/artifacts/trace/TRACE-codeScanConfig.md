---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4ab63678a55",
  "title": "Per-Project source-path scan configuration",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 2g6UVxVsxwm_ZJ0Rf1fKmb7CCJoJZqhR0_OI9hmG3D8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.17",
  "legacy": {
    "doorstopUid": "TRACE-codeScanConfig"
  }
}
---
Each Project's reqforge.json may declare a list of source paths
for the scanner to walk. When no list is declared, the scanner
shall fall back to a small set of sensible defaults derived from
common conventions (for example, src/, tests/, and lib/). The
scanner shall walk each declared path recursively, filter files
by the extension globs registered in the language registry, and
exclude common ignore directories (.git, node_modules, target,
dist, build, __pycache__, and .venv, at minimum).
