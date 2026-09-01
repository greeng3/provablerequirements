---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df3abed4fbf4",
  "title": "Collection configuration schema",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: CgqhMJ5XrqeVyamz98o6nTYT82-tECPLLw71noRzWuM="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.11",
  "legacy": {
    "doorstopUid": "FORMAT-collectionConfigSchema"
  }
}
---
A Collection's .collection.json file shall contain at minimum:
  - schemaVersion (integer): the collection-config schema
    version.
  - prefix (string): the Collection's artifact-ID prefix (for
    example, STOR), unique within a Project.
  - name (string): a human-readable display name.
Optional fields include:
  - description (string): prose describing the Collection.
  - expectsCodeTrace (boolean, default true): per
    TRACE-codeCoverageExpectation, controls whether this
    Collection's artifacts are expected to have corresponding
    references in both source code and test files.
  - importNotes (object): opaque container for document-level
    metadata preserved from an importer (for example, the
    doorstop import may store the source document's parent
    prefix here as {"doorstopParent": "REQ"}). ReqForge does
    not interpret the contents; the field is preserved
    verbatim across reads and writes.
