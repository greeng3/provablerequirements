---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df6375b0b91b",
  "title": "Content-hosted artifacts as Markdown with JSON frontmatter",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: 3i6_C0O7fAArKlVGHEnldHKRt_esAi5f_WwIUjz8TlE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.2",
  "legacy": {
    "doorstopUid": "FORMAT-contentHostedMarkdown"
  }
}
---
Each content-hosted artifact shall be stored as a single Markdown
file (extension .md) whose body is the artifact's prose and whose
JSON frontmatter carries the artifact's metadata (UUID, title,
shape, review log, links, schema version, timestamps, and any
shape-specific fields). The frontmatter appears at the top of the
file delimited such that the body below renders as Markdown in
common viewers (GitHub, GitLab, and similar), even though those
viewers may not render the JSON frontmatter as richly as they
would render YAML frontmatter.
