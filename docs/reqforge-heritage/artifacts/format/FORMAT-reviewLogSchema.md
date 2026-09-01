---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0138cde3be6",
  "title": "Review log entry schema",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: EKeHHiakLlhBSnDi_jY8QzwQEitRpMDDei5PsCD-Xus="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.16",
  "legacy": {
    "doorstopUid": "FORMAT-reviewLogSchema"
  }
}
---
Each entry in an artifact's reviewLog array shall have:
  - timestamp (string, UTC ISO 8601 with trailing "Z",
    required).
  - reviewer (string, required): identifier of the reviewer,
    per REVIEW-reviewerIdentity.
  - outcome (string, required): one of "approved", "rejected",
    "todo-added", "todo-resolved", or another outcome tag the
    review workflow recognises as it evolves.
  - explanation (string, optional): prose accompanying the
    event.
  - addedTodos (array of objects, optional): each with id
    (string) and text (string), representing TODOs newly added
    by this event.
  - resolvedTodos (array of strings, optional): the ids of TODOs
    resolved by this event.
