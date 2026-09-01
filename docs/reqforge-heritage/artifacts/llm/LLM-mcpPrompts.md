---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1442ab86237",
  "title": "MCP canned-workflow prompts",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: BGAn1GKz84a0T8JMXROOKapb9M046ZLu48mKw4WsnLY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.17",
  "legacy": {
    "doorstopUid": "LLM-mcpPrompts"
  }
}
---
The MCP server shall ship a baseline set of canned prompts
representing common workflows an AI coding agent is likely to
perform against a ReqForge System, at minimum:
  - Gap analysis: identify requirements with no satisfying or
    verifying children.
  - Coverage summary: summarise coverage across a Project or
    Collection, calling out gaps and partial coverage.
  - Review assist: help the human review an artifact against
    its dependents and its since-last-approval changes (per
    UX-reviewPane).
  - Implementation planning: given a set of requirements
    (typically those flagged as gaps by gap analysis), draft
    an implementation plan that respects the existing link
    graph and review state.
  - Test / verification gap planning: given a requirement
    lacking a verifier, draft a test or verification artifact
    that would close the gap (per ART-verificationConvention).
  - Impact analysis narrative: given a proposed change to an
    artifact, summarise the set of artifacts transitively
    dependent on it and what re-review would likely entail.
The prompt set is extensible; additional prompts may be
added in future releases without breaking the tool or
resource surface.
