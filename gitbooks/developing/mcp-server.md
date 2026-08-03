---
description: Run Aura Core as a read-only stdio Model Context Protocol server.
icon: plug
---

# MCP Server

Aura Core can run as an opt-in stdio MCP server for local MCP clients such
as Claude Desktop, Cursor, or Zed.

```bash
aura-core mcp
```

The command does not start the HTTP JSON-RPC server. It reads newline-delimited
JSON-RPC 2.0 messages from stdin and writes MCP responses to stdout. Logs go to
stderr; add `--verbose` for debug output.

## Tools

The MCP surface is deliberately read-only and routes through the existing
controller registry plus the core security policy read gate:

| MCP tool | Backing RPC | Purpose |
| --- | --- | --- |
| `memory.search` | `aura.memory_tree_search` | Keyword search over memory-tree chunks. |
| `memory.recall` | `aura.memory_tree_recall` | Semantic recall over memory-tree summaries/chunks. |
| `tree.read_chunk` | `aura.memory_tree_get_chunk` | Read one chunk returned by search or recall. |
| `tree.browse` | `aura.memory_tree_list_chunks` | Paginated chunk listing with source / entity / time filters. |
| `tree.top_entities` | `aura.memory_tree_top_entities` | Most-referenced canonical entities, optionally filtered by kind. |
| `tree.list_sources` | `aura.memory_tree_list_sources` | Distinct ingest sources with chunk counts and last-activity timestamps. |

`memory.search` and `memory.recall` accept `query` plus optional `k` (default
10, capped at 50). `tree.read_chunk` accepts `chunk_id`. `tree.browse`
accepts optional `source_kinds`, `source_ids`, `entity_ids`, `since_ms`,
`until_ms`, `query`, `k`, and `offset`. `tree.top_entities` accepts optional
`kind` and `k`. `tree.list_sources` accepts an optional `user_email_hint`.

## Smoke Test

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  | aura-core mcp
```

The response should include `capabilities.tools` from `initialize` and all six
tool names from `tools/list`. A successful run writes exactly two compact JSON
response lines to stdout; the `notifications/initialized` message is a
notification and has no response.

```text
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"aura-core","version":"<crate version>"},"instructions":"..."}}
{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"memory.search",...},{"name":"memory.recall",...},{"name":"tree.read_chunk",...},{"name":"tree.browse",...},{"name":"tree.top_entities",...},{"name":"tree.list_sources",...}]}}
```
