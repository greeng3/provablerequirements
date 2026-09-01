---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e6759716d59e",
  "title": "Large-list rendering",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 4Q9FbqS96upj3_XQ0TgSb7sFyZacrJp5neqpQv3ES9A="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.25",
  "legacy": {
    "doorstopUid": "UX-largeListRendering"
  }
}
---
Any UI list whose length may exceed approximately 200 entries
— including Collection artifact lists, search result lists,
the System-wide review queue, report tables, and bulk-action
selection lists — shall use viewport-based virtualised
rendering (for example, via TanStack Virtual or an equivalent
MIT-licensed React virtualisation library). The user
experience is continuous scrolling; ReqForge shall not expose
explicit numeric pagination controls. This matches the matrix
view's rendering approach (per UX-linkCreationMatrix) for
consistency across the UI.
