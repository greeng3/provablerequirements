---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e4c40c6b4a1a",
  "title": "Code tag syntax",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: -3S1tTlSID49MqOV00loWcya5vhyfcADxeIjQxsGs6s="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.15",
  "legacy": {
    "doorstopUid": "TRACE-codeTagFormat"
  }
}
---
Requirement tags in source code shall be recognised only within
comments. Each tag takes the form "<Verb>: <id>[, <id>]..." where
<Verb> is the name of a built-in link type (Satisfies, Verifies,
Derives-From, Supersedes, Conflicts-With, Related-To) or an
accepted alias. Implements and Requirements shall be accepted as
aliases for Satisfies, reflecting the natural verbs used in
source comments. A tag may list multiple comma-separated IDs, and
a trailing comma on a tag line shall cause the list to continue
onto subsequent comment-only lines carrying bare IDs.
