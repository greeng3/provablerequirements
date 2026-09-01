---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e204b8e91a7e",
  "title": "LLM API keys stored in the System config",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: TfHo1WHfpmXDl3rKfUPgMiJxPvHpgTjzBR2oEZ_8CGA="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.4",
  "legacy": {
    "doorstopUid": "LLM-secretsViaEnv"
  }
}
---
API keys for LLM providers shall be stored directly in the
System configuration file under the relevant provider entry's
optional apiKey field. ReqForge shall not require operators to
manage environment variables for keys. The System config lives
in the operator's local workspace, outside any tracked Project
repository; on POSIX hosts the loader shall reject System
config files whose mode is world-readable so file-system
permissions alone keep stray secrets from leaking. Operators
who prefer to keep keys out of any on-disk file may run with
no apiKey set and rely on a keyless provider (e.g. a local
Ollama instance), but ReqForge does not offer indirection
through environment variables as a first-class path.
