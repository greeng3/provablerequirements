---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1b81c5827e9",
  "title": "Post-doorstop-import rename suggestion",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: JMnaCd3kTgEmETgRN9jg702REzAJPgPAN3B3_9VCVvk="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.10",
  "legacy": {
    "doorstopUid": "LLM-postImportRenameSuggest"
  }
}
---
When a doorstop import completes (per INTEROP-doorstopImport)
and at least one LLM provider is configured and healthy, the
import report shall offer a "Suggest better names for imported
artifacts" action, using LLM-renameWorkflow. If the user
declines at that moment, the suggestion action shall remain
available later from each imported artifact's menu and as a
bulk action on the imported Collections.
