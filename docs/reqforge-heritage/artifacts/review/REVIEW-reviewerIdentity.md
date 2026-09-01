---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e34a2a85a950",
  "title": "Weak reviewer identity",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: AQwimRS3UbYrIVVQu1oWqR7cx5uto2lCehd0LYKkH6w="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "REVIEW-reviewerIdentity"
  }
}
---
Pending a full authentication system, ReqForge shall use a
weak identity scheme for the reviewer field on review-log
entries:
  - The default identity is the repository's configured git
    user.name, read from the repository's .git/config file
    (a plain INI-format file that ReqForge can parse directly
    without invoking git or linking a git library).
  - Before submitting a review action, the UI shall present a
    dropdown whose initial selection is the git-config
    default. The dropdown's other entries are identities
    previously used in the current container lifetime and
    identities previously persisted to a reviewers.json file
    inside the workspace directory (.reqforge-workspace/
    reviewers.json; per DEPLOY-operatorWorkspace and
    DEPLOY-devWorkspace).
  - The user may type a new identity string; on review
    submission, the new identity is appended to reviewers.json
    so it appears in future dropdowns.
The scheme is explicitly non-authenticating: the stored
identity is claimed, not verified. Multi-user authentication
and authorization are deferred and shall, when added,
supersede this mechanism without changing the review-log
schema.
