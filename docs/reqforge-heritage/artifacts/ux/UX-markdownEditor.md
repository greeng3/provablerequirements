---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e6becd449503",
  "title": "Side-by-side Markdown authoring editor",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: NZJYqrrEDIFOWdvMIXBy1HVjaNT5VBQFEzkjc_6BuWo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "UX-markdownEditor"
  }
}
---
Content-hosted artifacts shall be authored in a side-by-side view
with a text-editor pane on one side and a live-rendered Markdown
pane on the other. The text pane is the single source of truth
for the artifact body; the rendered pane updates from it in real
time as the user types. This one-directional flow preserves
hand-written text fidelity — indentation, HTML comments, soft
line breaks, and list-style choices remain exactly as written —
without the normalisation that round-tripping WYSIWYG editors
impose. The implementation shall use a CodeMirror 6 text editor
(with Markdown syntax highlighting) and a react-markdown
renderer, both free and MIT-licensed.
