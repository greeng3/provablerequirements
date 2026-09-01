---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df926933f4dd",
  "title": "Frontmatter delimiter convention",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: tPnA1kftmHASoopqUVo1DxU2d6Fwj1ot1PkyJoqaHOQ="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.8",
  "legacy": {
    "doorstopUid": "FORMAT-frontmatterDelimiters"
  }
}
---
The JSON frontmatter of a content-hosted artifact's .md file shall
be delimited by YAML-style triple-dash markers: a line containing
exactly "---" opens the frontmatter, the JSON object occupies the
lines between the delimiters, and a closing "---" line precedes
the Markdown body. Because any valid JSON is also valid YAML
flow-style, Markdown renderers that expect YAML frontmatter
(GitHub, GitLab, Pandoc, Jekyll, Hugo, and similar) parse and
display the block correctly without a ReqForge-specific extension.
