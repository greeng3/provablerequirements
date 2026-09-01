---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-dfa11f2d2234",
  "title": "Identifier validation rules",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: KPETfy8UKzcx9m5j5KNN4je5GmfOWXIEVlsW9v9bWDU="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.18",
  "legacy": {
    "doorstopUid": "FORMAT-inputValidation"
  }
}
---
Project slugs, Collection prefixes, and artifact names shall
follow ASCII C-identifier rules: the identifier begins with an
ASCII letter or underscore, and subsequent characters are ASCII
letters, digits, or underscores, matching the regular
expression ^[A-Za-z_][A-Za-z0-9_]*$. ReqForge shall validate
these fields at input time in the UI and again at load time
when reading on-disk configuration or artifact metadata;
invalid values produce a clear error rather than silent
tolerance. Conventions beyond the regex — uppercase Collection
prefixes (STOR, ART), camelCase artifact names (gitNative,
systemAboveProject), lowercase slugs — are stylistic and are
not enforced by validation.
