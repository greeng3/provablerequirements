---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0e4b51228d5",
  "title": "Synthetic initial review from doorstop reviewed hash",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: rg0ZudQi2Vk0blX8PY9y0NmCejc6rQ9KZD43jMEXVVY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopReviewedHash"
  }
}
---
When a doorstop item carries a non-null reviewed hash, the
importer shall produce a synthetic initial review-log entry on
the imported artifact with:
  - outcome: "approved"
  - reviewer: "imported-from-doorstop"
  - timestamp: the import-run time (ISO 8601)
  - explanation: a short note recording the original reviewed
    hash (for example, "Imported from doorstop; original
    reviewed hash: <hash>").
Items whose reviewed field is null at import time produce an
empty review log; they appear as unreviewed in ReqForge until a
reviewer acts on them.
