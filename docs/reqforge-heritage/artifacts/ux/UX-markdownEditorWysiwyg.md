---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e6caa492453e",
  "title": "WYSIWYG Markdown editing (future)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: C3VP69b_V7llxUYwHfPXBhTdA49b7u32IYG-om5ZaAc="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.16",
  "legacy": {
    "doorstopUid": "UX-markdownEditorWysiwyg"
  }
}
---
A bidirectional WYSIWYG editing mode for content-hosted artifacts
— where editing in the rendered pane updates the text pane via a
shared abstract-syntax-tree editor (ProseMirror-based, for
example Milkdown or TipTap with a Markdown extension) — is
deferred. Shipping it means accepting that Markdown round-trips
through the AST will normalise hand-written text (list styles,
HTML comments, indentation, and similar) to the AST's canonical
form. The initial Markdown editor (UX-markdownEditor) is
one-directional precisely to avoid that normalisation; the
WYSIWYG mode would be an opt-in second editing mode, not a
replacement.
