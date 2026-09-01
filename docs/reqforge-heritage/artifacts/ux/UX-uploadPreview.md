---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e7a21d339251",
  "title": "Upload preview tiers by format",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: RPjoizgfyRsAo6ANUIcix5jDMAENWxtkh7ull5Rziu0="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.17",
  "legacy": {
    "doorstopUid": "UX-uploadPreview"
  }
}
---
Preview of uploaded-blob artifacts in the UI shall be tiered by
how comfortably the browser can render the format:
  - Browser-native formats (PDF, common image formats, SVG,
    plain text): embedded inline using standard HTML elements,
    with no server-side preprocessing required.
  - Microsoft Office and other complex binary formats for which
    ReqForge's container ships a thumbnailer: the thumbnail is
    generated on first view (lazy/on-demand), cached on disk
    next to the blob as a sibling file with a well-known suffix
    (for example, DES-spec.pdf alongside
    DES-spec.pdf.reqforge-thumbnail.png), and reused for
    subsequent views. Invalidation is keyed on the blob's
    content hash.
  - Formats ReqForge cannot currently thumbnail: a generic
    file-type icon plus a surfaced note to the user that
    ReqForge lacks a thumbnailer for this format, along with
    the filename and size; download-only.
The container image shall include thumbnailers for a
reasonable common set of formats (for example, LibreOffice
headless for Office documents, ImageMagick or libvips for
additional image formats). The image is not split into
slim/full variants; common-format coverage is shipped
uniformly. Operators or users who need a thumbnailer for an
additional format contribute it back to the project rather
than overriding it locally.
