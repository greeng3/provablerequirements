---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e61c148f2627",
  "title": "Empty-state UI guidance",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: _lLkdXNRgzrIHgdLtJFEUsKdE0JJfV0I9UPdbvPWtYw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.34",
  "legacy": {
    "doorstopUid": "UX-emptyStates"
  }
}
---
Every ReqForge view that can be empty shall render an
explanatory message or a clear call-to-action rather than a
blank pane. The message shall distinguish between:
  - "Nothing exists yet" states — inviting creation (for
    example: no Projects mounted, no Collections in this
    Project, no artifacts in this Collection, no review-queue
    items). These surfaces offer the relevant creation action
    or a pointer to the next onboarding step.
  - "Nothing matches your current filters" states — inviting
    the user to relax the filter or scope (for example: no
    search results for the current query, no reports visible
    under the current scope selector). These surfaces suggest
    broadening the criteria rather than creating new content.
Empty states shall never surface a bare message like "no
items" without further guidance.
