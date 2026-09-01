---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-deaad9dcd610",
  "title": "Filesystem polling for external changes",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 4B-9PTsoVVwEj6Qa6g2vRwhEB3Pw2N0OJsn-qdaiJdI="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.8",
  "legacy": {
    "doorstopUid": "DEPLOY-pollingWatch"
  }
}
---
ReqForge shall detect external changes to mounted repositories —
for example, from git pull or the user editing files outside
ReqForge — and reflect them in the UI. Detection shall use polling,
because inotify and related mechanisms do not reliably cross
bind-mount boundaries on all host platforms.
