---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e7be133c07d2",
  "title": "URL artifact checking",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: _mN60VtuY3c3vLiECmIWiuv1rqPWCXlJNxBxFeDespo="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.24",
  "legacy": {
    "doorstopUid": "UX-urlArtifactChecking"
  }
}
---
ReqForge shall offer the user a "Check URL now" action on every
URL-reference artifact, and an equivalent bulk action on any
Collection or filtered list of URL artifacts. The action
performs a single HTTP request against the stored url and
updates two metadata fields on the artifact:
  - checkedAt: the UTC ISO 8601 timestamp of the attempt.
  - checkStatus: the outcome of the attempt, recorded as a
    short stable string ("ok" for 2xx responses, and values
    such as "not-found", "server-error", "timeout",
    "connection-refused", "tls-error" for failure modes; the
    raw status line or error message may be retained in the
    review log or artifact description at the user's
    discretion).
ReqForge shall not check URLs on any automatic schedule;
checks happen only when the user explicitly requests them.
Failure outcomes do not mark the artifact inactive or block
any operation; they are informational.
