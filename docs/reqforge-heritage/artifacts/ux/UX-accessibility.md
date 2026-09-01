---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e5ab567b229d",
  "title": "Accessibility by construction",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Zq7KbpXQEHLl38ExIRZJ4mdsNbcZVcPrjABZ0KYtrpk="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.28",
  "legacy": {
    "doorstopUid": "UX-accessibility"
  }
}
---
ReqForge's UI shall be accessible by construction rather than
by post-hoc audit:
  - Semantic HTML elements are used where they exist (button,
    nav, main, aside, table, and similar) rather than generic
    div or span.
  - Keyboard focus follows sensible reading order; all
    interactive affordances are reachable and operable via
    keyboard.
  - ARIA attributes (roles, labels, live regions) are added
    where native semantics are insufficient.
  - Visible focus indicators are preserved, not suppressed by
    styling.
Formal WCAG 2.1 AA certification, screen-reader QA passes,
and assistive-technology audits are explicitly not committed
for the initial version; accessibility is treated as
"reasonable by default" rather than "verified compliant."
This scope matches the target audience (per Scope and
Audience) and can be revisited when a concrete accessibility
need emerges.
