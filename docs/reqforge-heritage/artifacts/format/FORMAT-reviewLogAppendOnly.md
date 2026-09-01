---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e003997c6eb1",
  "title": "Review log is append-only; current state is derived",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 3VU4L4xhzMzMS29Kmv7ClmQYdTkK3I5In4VUJz1JGU4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.17",
  "legacy": {
    "doorstopUid": "FORMAT-reviewLogAppendOnly"
  }
}
---
The review log shall be append-only: existing entries shall not
be mutated or removed. The artifact's current review state
(current outcome, the set of open blocking TODOs, and similar
derived facts) shall be computed by walking the log in timestamp
order. This provides an immutable audit trail and avoids mutable-
state divergence; the computational cost of replaying a log is
trivial at any realistic review-log size.
