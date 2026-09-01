---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e715cd2d08b0",
  "title": "Per-artifact review pane",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: BmICfj413LVupdslGVEYfTgBg_fzjR0qVg1QYLCvP98="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "UX-reviewPane"
  }
}
---
Each artifact shall have a review pane in the UI showing its
review log (reviewer, timestamp, outcome, explanation) and its
current unresolved blocking TODOs. The pane shall also surface a
"Since last approval" section covering the period between the
artifact's most recent approved review-log entry and the current
state; this section has two sub-panels:
  - A content diff showing what has changed in the artifact's
    body and metadata since the last approval.
  - A review activity timeline listing every review-log entry
    between the last approval and now, in chronological order,
    showing each entry's timestamp, outcome, reviewer, and any
    TODOs added. Resolved TODOs render with a strike-through
    or equivalent "resolved" badge; unresolved TODOs are
    visually highlighted with a pending indicator.
If the artifact has never been approved, the pane displays the
full content under a banner noting "No prior approval — this is
the first review" and omits the activity timeline.
