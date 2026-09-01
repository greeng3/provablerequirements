---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e10b37bf227a",
  "title": "Requirements extraction from code and tests (future)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Zob3yT49bCg5imH5RC3hk6oYtwrWsplppzxEMCvmnnY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "LLM-codeScanExtraction"
  }
}
---
ReqForge shall eventually support LLM-assisted extraction of
proposed requirements and design documents from a repository
containing no existing ReqForge-managed requirements. The
extractor shall read source code, tests, and any ambient
documentation (READMEs, architectural notes, inline prose) and
propose candidate artifacts for user review. Output quality is
not expected to match careful human authorship; the goal is a
first-draft that removes the blank-page burden. The feature is
deferred.
