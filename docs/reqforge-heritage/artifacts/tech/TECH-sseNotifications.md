---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e477a4c4891b",
  "title": "Server-sent events for change notifications",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: tpJatADKM2koBSoyRLql7GvzpI1zJl8B3TfcwQz9Wjg="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "TECH-sseNotifications"
  }
}
---
ReqForge shall push filesystem-change notifications (from the
polling watcher of DEPLOY-pollingWatch) to connected front-end
instances via Server-Sent Events (SSE). SSE is chosen over
WebSocket for its simpler failure model, built-in browser
reconnection, and unidirectional server-to-client direction,
which matches change-notification semantics exactly.
WebSocket is explicitly deferred as a later option if a future
feature requires bidirectional streaming.
