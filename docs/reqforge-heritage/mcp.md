# Using ReqForge with AI coding agents

ReqForge ships `reqforge-mcp`, a small binary that speaks the
[Model Context Protocol](https://modelcontextprotocol.io/) over stdio.
Point any MCP-aware coding agent at it and your agent can read
ReqForge artifacts, walk the traceability graph, run reports, and
load workflow prompts — all without parsing raw files or
screen-scraping the web UI.

The MCP server is **read-only** in this release. AI-driven writes
are deferred until the read-only surface is proven in practice.

## What your agent gets

- **Eleven tools** that map to ReqForge's REST endpoints:
  list/get projects, collections, artifacts; incoming links; full-
  text search; report runs (all nine kinds); graph walks.
- **One resource per artifact** at `reqforge://artifact/{uuid}` —
  agents can reference an artifact by URI and read its full body
  (title, path, UUID, shape, tags, markdown) directly into
  context.
- **Six canned workflow prompts** — gap analysis, coverage summary,
  review assist, implementation planning, test-gap planning, and
  impact-analysis narratives. Each one tells the agent which tools
  to call and how to structure its response.

## Prerequisites

1. ReqForge running locally. `make dev` works, as does a Docker
   image bound to the default port:

   ```sh
   make dev                 # dev mode, binds to :36743
   # or
   make docker-run          # production container image
   ```

2. The `reqforge-mcp` binary available on the host. Build it from
   the workspace:

   ```sh
   cargo build --release -p reqforge-mcp
   # installs at backend/target/release/reqforge-mcp
   ```

   For convenience, place the binary on `PATH`:

   ```sh
   cargo install --path backend/reqforge-mcp
   ```

## Wiring into the agent

Each coding agent has its own config file for MCP servers. Three
common ones follow; every MCP-aware agent supports this pattern
(a command to spawn + args + optional env), so adapt as needed.

### Claude Code

Edit `~/.config/claude-desktop/claude_desktop_config.json` (or the
equivalent settings surface on your platform) and add a `reqforge`
entry under `mcpServers`:

```jsonc
{
  "mcpServers": {
    "reqforge": {
      "command": "reqforge-mcp",
      "args": [],
    },
  },
}
```

If ReqForge isn't on the default port or isn't on localhost,
pass `--url`:

```jsonc
{
  "mcpServers": {
    "reqforge": {
      "command": "reqforge-mcp",
      "args": ["--url", "http://127.0.0.1:36800"],
    },
  },
}
```

### Cursor

In `~/.cursor/mcp.json` (or the workspace-scoped `.cursor/mcp.json`):

```jsonc
{
  "mcpServers": {
    "reqforge": {
      "command": "reqforge-mcp",
    },
  },
}
```

### Zed

In `~/.config/zed/settings.json`, under `assistant.mcp_servers`:

```jsonc
{
  "assistant": {
    "mcp_servers": {
      "reqforge": {
        "command": "reqforge-mcp",
        "args": [],
      },
    },
  },
}
```

## Options

`reqforge-mcp` is intentionally a thin adapter; it has only two
runtime flags.

| Flag               | Default                  | Purpose                                                                                                                              |
| ------------------ | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| `--url <URL>`      | `http://127.0.0.1:36743` | Base URL of the `reqforge-server` to proxy into.                                                                                     |
| `--allow-remote`   | _off_                    | Permit non-loopback `--url` values. Without this flag, the binary refuses anything that isn't `localhost` / `127.0.0.0/8` / `[::1]`. |
| `--help` / `-h`    | —                        | Usage summary.                                                                                                                       |
| `--version` / `-V` | —                        | Binary version + MCP protocol version.                                                                                               |

Logs go to stderr (stdout is reserved for JSON-RPC traffic).
`RUST_LOG=reqforge_mcp=info` surfaces the per-request detail.

## Privacy

The default loopback-only posture is deliberate: requirements can
contain commercially sensitive detail, and pointing an agent at a
remote ReqForge instance by accident shouldn't be easy.
`--allow-remote` exists for the case where you're explicitly
running the agent on a different host to the ReqForge instance and
know what you're doing.

Operator acknowledgement applies at the ReqForge LLM-provider layer
(Phase 10a's privacy warning) — the MCP surface itself doesn't
further gate the agent's access, since the agent and ReqForge are
assumed to be on the same host per ReqForge's single-user-localhost
posture.

## Example session

After wiring in Claude Code:

```
> Summarize coverage across the whole system.

I'll fetch the coverage-matrix report.
[tool] reqforge_run_report { "kind": "coverage-matrix" }
[tool response] { totalParents: 42, gapCount: 5, … }

The system has 42 parent artifacts tracked by the default covering
link set (`satisfies`, `verifies`). Five have no covering children:
three under REQ and two under UC …
```

The agent can also follow a resource:

```
> Can you read REQ-pressure-envelope?

[resource] reqforge://artifact/11111111-1111-1111-1111-111111111111
[contents] # Pressure envelope
**Path:** `sample/REQ/REQ-pressure-envelope`
…
```

## Troubleshooting

- **Agent says the server isn't running.** Check the binary is on
  `PATH` (`which reqforge-mcp`) and that it runs standalone:

  ```sh
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | reqforge-mcp
  ```

  You should see a JSON response with `protocolVersion` and
  `serverInfo`.

- **Tool calls fail with HTTP errors.** Make sure `reqforge-server`
  is running and accessible at `--url`:

  ```sh
  curl http://127.0.0.1:36743/healthz
  ```

- **`refusing to connect to non-loopback URL`.** You pointed
  `--url` at a remote host; pass `--allow-remote` if that's
  intentional.

- **Agent can't find resources.** The agent may cache the resource
  list. Most clients have a "reload" or "reconnect" action; after
  editing artifacts in ReqForge, re-run that so the agent picks up
  the new set.

## What's not in this release

- **Write tools** (create / edit / delete artifacts, submit
  reviews). Deferred until the read-only surface has operated in
  practice. When writes land, they'll route through the existing
  review workflow the same way any other drafted artifact does.
- **Streamable HTTP transport.** Stdio-only for now; every major
  coding agent supports stdio today. Streamable HTTP lands if a
  concrete use case drives it.
- **Subscription notifications.** Resources use the poll-for-fresh
  model. Clients call `resources/list` when they want an updated
  snapshot.
