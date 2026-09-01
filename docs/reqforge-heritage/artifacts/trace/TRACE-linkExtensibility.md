---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e527bcd57462",
  "title": "System-level link type extensibility",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: fFt3ER8B2FligRHV1jfuQKqcNaZOffc-eBeZkwGp3WI="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "TRACE-linkExtensibility"
  }
}
---
The System configuration file may declare additional link types
beyond the built-in catalog. Each declared type shall carry the
same metadata shape as a built-in type: forward name, inverse
name, directedness flag, and acyclicity flag. Built-in link
types are always available and shall not be overridden by
System-level declarations.
When an artifact's link uses a type that is neither a built-in
nor declared in the currently active System's configuration
(for example, because the link was imported from, or written
in, a repository whose home System declared a type this System
does not), ReqForge shall gracefully degrade: the link remains
fully readable — target UUID, hint, and stored type name are
displayed — with a visible "unknown link type" indicator. The
link is not deleted or rewritten. Reports may surface unknown
link types as a cleanup category. Users who want full semantic
support for such a type can add it to the current System's
configuration.
