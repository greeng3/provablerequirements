---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e66142963796",
  "title": "Keyboard shortcuts limited to basic editor shortcuts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: nEEzWhIT6JIzEshOuQ6MhXoDNpMteKAC1RuNoe3pqBE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.27",
  "legacy": {
    "doorstopUid": "UX-keyboardShortcuts"
  }
}
---
The initial version of ReqForge shall ship only the keyboard
shortcuts a user reasonably expects from a basic Markdown
editor:
  - Standard text-editing shortcuts inherited from the editor
    library (cursor movement, selection, cut/copy/paste,
    undo/redo, find/replace, and similar) come for free from
    CodeMirror.
  - Ctrl+S / Cmd+S explicitly triggers save on the current
    artifact.
ReqForge-specific navigation shortcuts (focus search, jump
between artifacts, open modals, and similar) are deferred
until specific shortcuts are requested by users with concrete
need. Configurable bindings are likewise deferred.
