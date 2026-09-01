---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df8ef129b75e",
  "title": "File encoding and formatting conventions",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: LRD4dcKaZcp8RX187umbyd6PZE-GGA1PLe77bxAduHM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.20",
  "legacy": {
    "doorstopUid": "FORMAT-fileEncoding"
  }
}
---
All ReqForge-authored files shall be UTF-8 encoded without a
byte-order mark — Markdown artifact bodies, JSON frontmatter,
configuration files, and sidecars alike. Line endings on
write shall be LF only; on read, CRLF sequences shall be
tolerated and normalised to LF in memory without rewriting
the file. JSON — whether as standalone files or as the
frontmatter block in a content-hosted Markdown artifact —
shall be written pretty-printed with two-space indentation
and a trailing newline, optimising for small, readable git
diffs rather than compact bytes.
