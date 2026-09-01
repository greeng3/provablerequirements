---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e1e3a4e33ce4",
  "title": "Provider adapter families",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: WRf4pKXs7XV9U_-X_o_htaHp_kCGXeJ8Z1mQ-HUdQfY="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.2",
  "legacy": {
    "doorstopUid": "LLM-providerAdapters"
  }
}
---
ReqForge shall ship at least three LLM provider adapter
families in its initial version:
  - OpenAI-compatible: covers OpenAI, Azure OpenAI, Ollama
    (local), LMStudio (local), vLLM, llama.cpp server,
    OpenRouter, LiteLLM proxy, and any other service that
    exposes the OpenAI Chat Completions or Responses API with
    a configurable base URL.
  - Anthropic: native Claude API.
  - Google Gemini: native Gemini API.
Additional adapter families may be added in future releases
without breaking existing System configurations.
