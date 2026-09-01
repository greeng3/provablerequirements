---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-df073343f4b4",
  "title": "Artifact metadata core schema",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: jJpJxt2NADEIKRL3vMYGX-YZsSHx870DPHiI1TitzOY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.13",
  "legacy": {
    "doorstopUid": "FORMAT-artifactMetadataSchema"
  }
}
---
Every artifact's metadata — whether carried in JSON frontmatter of
a content-hosted .md file, a blob sidecar .reqforge.json file, or
a URL artifact .reqforge.json file — shall include at minimum:
  - schemaVersion (integer): the artifact-metadata schema
    version. This single version governs the entire artifact
    file including all nested structures (links, review log
    entries, TODOs); nested structures do not carry their own
    version numbers.
  - uuid (string): the artifact's stable identity, a UUIDv7
    (time-ordered) value in canonical lowercase hyphenated
    form. UUIDv7 is chosen for natural chronological ordering
    and index locality.
  - title (string): a human-readable title.
  - shape (string): one of "content", "blob", or "url".
  - createdAt (string, UTC ISO 8601 timestamp with trailing
    "Z", for example "2026-04-17T14:30:00Z"; all timestamps
    ReqForge writes shall be UTC).
  - modifiedAt (string, UTC ISO 8601 timestamp as above).
  - links (array): the typed outgoing links; see
    FORMAT-linkPayloadSchema.
  - reviewLog (array): the review event history; see
    FORMAT-reviewLogSchema.
Optional fields include:
  - description (string): short descriptive prose.
  - expectsCodeTrace (boolean): per-artifact override of the
    Collection default, per TRACE-codeCoverageExpectation.
  - active (boolean, default true): the artifact's active/
    inactive lifecycle flag, per ART-activeField.
  - derived (boolean, default false): flags artifacts derived
    from an external source rather than authored internally,
    per ART-derivedField.
  - tags (array of strings, default empty): free-form
    categorisation labels, per ART-tagsField.
  - outlineLevel (string): optional outline position (for
    example, "1.2.3") preserved from document-heritage
    workflows, per ART-outlineLevelField.
  - legacy (object): opaque container for unmapped fields
    carried in by importers, per ART-legacyField.
Shape-specific fields are defined in
FORMAT-artifactShapeSpecificFields.
