---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1398812979d",
  "title": "Per-provider health tracking and manual re-test",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: v0J69IhhfuMUL_mg6hPzEHM3mC3Skp6Y0o_y6g_edn4="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.6",
  "legacy": {
    "doorstopUid": "LLM-healthTracking"
  }
}
---
ReqForge shall track the health of each configured LLM provider
in memory for the lifetime of the container process:
  - Healthy: recent invocations succeeded; the provider is
    used normally.
  - Transient-degraded: the provider returned timeouts, rate
    limits, 5xx responses, or similar recoverable errors. The
    provider is skipped for an exponential-backoff window that
    lengthens on continued failure and shrinks on success.
  - Hard-disabled: the provider returned authentication
    failure (401/403), model-not-found (404), connection
    refused at the endpoint, or malformed responses. It is
    removed from the active rotation until the user intervenes.
A "Re-test providers" action in the UI shall retry any
hard-disabled entries so that a configuration fix can be picked
up without restarting the container; the resulting state
(healthy, transient-degraded, or still hard-disabled) is
recorded and returned.
