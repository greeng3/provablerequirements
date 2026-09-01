---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1143360681c",
  "title": "LLM configuration in the System config",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: _rM4rdHaLsI4VbXNwtM984V__SbWgH3G1zgd-px9qd8="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.3",
  "legacy": {
    "doorstopUid": "LLM-configSchema"
  }
}
---
The System configuration file shall carry an optional llm field
that is an array of provider configurations in priority order.
Each entry is an object with at minimum:

- provider (string): the adapter family, one of
  "openai-compatible", "anthropic", or "gemini".
- model (string): the model identifier interpreted by the
  chosen provider.
- endpoint (string, required for openai-compatible; optional
  for native providers when overriding the default host): the
  base URL the adapter targets.
- apiKey (string, optional): the API key the adapter sends.
  Omit for keyless providers (e.g. a local Ollama instance);
  provide for cloud providers that require authentication.
- enabled (boolean, optional, default true): when false, the
  fallback chain skips this entry. Disabling all but one
  yields "select one provider to use"; leaving several
  enabled yields fallback across them in priority order.

ReqForge shall walk this array in order when invoking any
LLM-dependent feature, skipping disabled entries (per
LLM-fallbackChain).
