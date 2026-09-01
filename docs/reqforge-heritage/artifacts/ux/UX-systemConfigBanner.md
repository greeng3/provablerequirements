---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e7863b2c8004",
  "title": "System configuration prompt",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: pWHucJWeNcUlThgCTd6d2exvibX_yKkAJ8QwCkOg4Vw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.22",
  "legacy": {
    "doorstopUid": "UX-systemConfigBanner"
  }
}
---
When two or more Projects are mounted but no System
configuration file has been loaded, the System Home shall show
a persistent but dismissible banner inviting the user to create
one, with a link to documentation describing the System config
format, the expected bind-mount pattern, and the
REQFORGE_SYSTEM_CONFIG environment variable. ReqForge shall not
write a System config file on behalf of the user; the action is
strictly user-driven.
