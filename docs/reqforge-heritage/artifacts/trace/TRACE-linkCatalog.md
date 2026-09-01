---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e50b0bb402fb",
  "title": "Built-in link type catalog",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: xHGTSGImTiJ8Jnh9Po8Y8wFbCmwNTZvgRTbdQWjP_Nk="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.9",
  "legacy": {
    "doorstopUid": "TRACE-linkCatalog"
  }
}
---
ReqForge shall provide a built-in catalog of seven link types as
the baseline for traceability:
  - derives-from (inverse derived-into; directed; acyclic): the
    source is a refinement of the target (child to parent).
  - satisfies (inverse satisfied-by; directed; no acyclicity
    constraint): the source fulfils the target.
  - verifies (inverse verified-by; directed; no acyclicity
    constraint): the source is evidence that the target holds,
    typically a test artifact relating to a requirement.
  - supersedes (inverse superseded-by; directed; acyclic): the
    source replaces the target.
  - cites (inverse cited-by; directed; no acyclicity
    constraint): the source references the target as an
    external or historical citation. The target is typically a
    URL artifact, a blob artifact holding an uploaded document,
    or a content-hosted artifact whose body is a bibliographic
    reference to a work not otherwise digitally available.
  - conflicts-with (self-inverse; symmetric; no acyclicity
    constraint): the pair is in unresolved conflict and needs
    attention.
  - related-to (self-inverse; symmetric; no acyclicity
    constraint): a weak, untyped association used as an escape
    hatch when no more specific type applies.
Each built-in type carries a forward name, an inverse name, a
directedness flag, and an acyclicity flag.
