---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e6e6341031ae",
  "title": "Performance targets (soft)",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: BNSH0--9TVMTNQYQ3QKPlCp5QlVdIZbC2-Mgv_84YAQ="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.30",
  "legacy": {
    "doorstopUid": "UX-performanceTargets"
  }
}
---
ReqForge shall treat the following as soft performance targets
on mid-range developer hardware, with user-perceptible
violations treated as bugs:
  - UI page transitions: 95th percentile under 200 ms.
  - API read-fetch round trips: 95th percentile under 50 ms.
  - Full-text search across roughly 10,000 artifacts: 95th
    percentile under 500 ms.
  - Cold UI load: under 3 seconds.
  - Markdown editor live-preview update, keystroke to render:
    under 100 ms.
These are targets, not hard service-level agreements; they
inform benchmarking, profiling, and issue triage rather than
the interactive contract. They apply within the scaling
targets of UX-scalingTargets; beyond those, performance is
best-effort.
