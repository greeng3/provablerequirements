---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e7952746a6ba",
  "title": "In-session undo and redo",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: pTu2xIsCwTE86hQ3dgJhy0mvEEnTyCfPHGmPfjCXDnY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.29",
  "legacy": {
    "doorstopUid": "UX-undoRedo"
  }
}
---
ReqForge shall provide in-session undo and redo for user
actions:
  - Markdown editor keystrokes (inherited from CodeMirror).
  - Link creation and deletion.
  - Artifact-level operations: create, delete, move between
    Collections, and rename.
The undo stack has a reasonable finite depth (on the order of
50 steps) and is scoped to the current tab / browser session.
Closing the tab, reloading the page, or saving-and-closing
clears the stack. Cross-session undo is not offered; git is
the cross-session undo mechanism (users revert via their git
client).
