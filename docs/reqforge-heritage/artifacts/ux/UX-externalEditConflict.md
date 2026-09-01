---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e62d9de13094",
  "title": "External-edit conflict handling",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Mez1B61uOZTlxqODZeOfrtTy79IZXwnc8WLHP4G8ljg="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.23",
  "legacy": {
    "doorstopUid": "UX-externalEditConflict"
  }
}
---
When the polling watcher (per DEPLOY-pollingWatch) detects that
an artifact file has been modified externally (for example, by
git pull or by the user's text editor) while the user has
unsaved changes to the same artifact open in the ReqForge UI,
the UI shall surface a conflict prompt offering three choices:
  - Keep my changes: abandon the external update in the UI's
    in-memory state; the user's next save overwrites the
    external version on disk.
  - Discard my changes and reload: replace the in-memory state
    with the external version, losing the user's unsaved edits.
  - Open merge diff: show a three-pane view — the user's
    in-progress edits, the external version, and a manual
    merge pane the user edits to produce the final saved
    result.
The prompt shall be unambiguous about which version is which
and shall not auto-dismiss on timeout; it shall remain until
the user explicitly chooses one of the three options.
