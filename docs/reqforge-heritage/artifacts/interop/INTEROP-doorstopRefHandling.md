---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0d19bae6acc",
  "title": "Doorstop ref-field handling",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: Vymb14L3hiaLxFCGGkhXKnhkrrJrty9dC6UpxACJZXM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "INTEROP-doorstopRefHandling"
  }
}
---
The doorstop ref field (an external-reference string, variously a
URL, a file path, or a bibliographic citation) shall be handled
according to its shape:
  - URL-shaped ref (detected by a URI scheme prefix such as
    https://, http://, ftp://, doi:, or urn:isbn:): the importer
    creates a new URL-reference artifact holding the URL, and
    adds a cites link from the imported artifact to the new URL
    artifact (per TRACE-linkCatalog).
  - Non-URL ref (file path, free-form citation, or anything else
    not recognisable as a URI): the value is preserved verbatim
    as the ref key inside the imported artifact's legacy object.
    The user may later convert it into a URL artifact, a
    content-hosted citation artifact, or a structured field via
    a one-time clean-up pass.
In both cases, the import report records how each ref was
handled.
