---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e12851ef1303",
  "title": "Priority-ordered fallback chain",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: gjm3j0aL6o1sKde2x2CdtmqJoRmWFG88BOc3dGagfvE="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.5",
  "legacy": {
    "doorstopUid": "LLM-fallbackChain"
  }
}
---
When an LLM-dependent feature is invoked, ReqForge shall attempt
providers in the order they appear in the llm array. The first
provider not currently marked hard-disabled or back-off-
throttled (per LLM-healthTracking) is tried; if its invocation
succeeds, the response is returned. On failure, ReqForge moves
to the next provider in the array. If every provider in the
chain is either throttled or fails, the feature shall fall back
to the plain action (for example, plain rename) where one
exists, or report the failure to the user where no plain
equivalent exists.
