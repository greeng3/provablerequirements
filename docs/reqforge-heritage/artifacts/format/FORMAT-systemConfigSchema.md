---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e0296faf260d",
  "title": "System configuration schema",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: qznjWRqnwwJhjO6orCiASoexAJxxBPOcWahaqpJf8rQ="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.12",
  "legacy": {
    "doorstopUid": "FORMAT-systemConfigSchema"
  }
}
---
The System configuration file shall contain at minimum:
  - schemaVersion (integer): the system-config schema version.
  - name (string): human-readable name of the System.
  - projects (array of objects, each with a slug string field):
    the Project slugs expected to belong to this System.
Optional fields include:
  - linkTypes (array): user-declared link types augmenting the
    built-in catalog. Each entry has name (string), inverseName
    (string), directed (boolean), and acyclic (boolean) fields,
    per TRACE-linkExtensibility.
  - languages (array): user-declared scanner languages
    augmenting the built-in registry. Each entry has name
    (string), extensions (array of strings), lineComments (array
    of strings), and blockComments (array of objects with start
    and end strings), per TRACE-codeLanguageRegistry.
  - llm (array): priority-ordered LLM provider configurations
    (per LLM-configSchema). Each entry has provider (string),
    model (string), endpoint (string, for openai-compatible and
    optional for native providers), and apiKeyEnvVar (string,
    optional) fields. Used by LLM-dependent features per
    LLM-fallbackChain.
