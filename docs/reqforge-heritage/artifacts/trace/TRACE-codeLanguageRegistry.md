---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e49ed5514128",
  "title": "Per-language comment registry",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: HbygM4--1-_3Adq1q385n6gu2KOMVskzVBMvogPu3Xg="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.14",
  "legacy": {
    "doorstopUid": "TRACE-codeLanguageRegistry"
  }
}
---
ReqForge shall maintain a registry of supported source languages.
Each registry entry declares the language's file-extension globs,
line-comment markers, and block-comment markers. The built-in
registry shall include at minimum:
  - Rust (.rs): line comments //, ///, //!; block comments /* */.
  - Python (.py): line comments #; triple-quoted strings
    ("""...""" and '''...''') treated as comments for the purpose
    of tag scanning.
  - JavaScript and TypeScript (.js, .jsx, .ts, .tsx): line
    comments //; block comments /* */ including /** */.
  - POSIX shell (.sh, .bash): line comments #.
  - Dockerfiles (files named Dockerfile or matching Dockerfile.*):
    line comments #.
The System configuration file may declare additional languages
in the same shape. YAML is deliberately omitted from the
initial built-in registry.
System-declared language entries shall be add-only: a
user-declared entry whose name matches a built-in is rejected
at configuration-load time with a clear error message pointing
the user at contributing the fix upstream rather than silently
overriding the built-in behaviour. Users wanting different
semantics for a built-in language shall file a bug or submit a
change against ReqForge itself.
