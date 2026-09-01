---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1c084a6949a",
  "title": "Privacy warning for cloud providers",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: IHparXDm4R0MyKl2My0st2sgLIT0LVlw0AQvHkJBf3M="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.7",
  "legacy": {
    "doorstopUid": "LLM-privacyWarning"
  }
}
---
Before ReqForge first transmits artifact content to a given
cloud LLM provider within a container lifetime, the UI shall
display a one-time warning naming the provider and noting that
artifact content will be transmitted off the local machine. The
user acknowledges once per provider per container lifetime;
restarting the container re-prompts, because the provider list
may have changed between runs. Providers whose endpoint
resolves to localhost or an RFC 1918 private-IP range shall be
treated as local and skip the privacy warning entirely.
