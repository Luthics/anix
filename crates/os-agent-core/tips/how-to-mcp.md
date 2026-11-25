# How to Connect to MCP Servers via POSIX Shell

OS Agent intentionally keeps MCP clients out of the core binary. Discover and call MCP servers on demand via `msh`.

## 1. Discover Server Configuration
Check project and global MCP configs with `jq`:
```sh
for f in ./.mcp.json ./.agent/mcp.json "${XDG_CONFIG_HOME:-$HOME/.config}/agent/mcp.json"; do
  [ -f "$f" ] && jq '{file: "'"$f"'", servers: (.mcpServers // {} | keys)}' "$f"
done
```

## 2. Prefer an Installed MCP CLI Bridge
If `mcporter` or `mcp-cli` is available, inspect and call tools in one command and trim large schemas with `jq`:
```sh
mcporter list --json | jq 'map({name, tools: (.tools | map(.name))})'
mcporter call <server>.<tool> key=value
```

## 3. Zero-Dependency POSIX Fallback (JSON-RPC 2.0)
- **Stdio MCP Server** (close `stdin` and bound with `timeout` to prevent hangs):
```sh
timeout 15 <server-cmd> <<'EOF' | jq -c 'select(.id == 2)'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"os-agent","version":"1.1"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
EOF
```
- **Streamable HTTP / SSE MCP Server** (`curl`, carrying `Mcp-Session-Id` & auth header as needed):
```sh
curl -fsS -X POST "$MCP_URL" -H 'Content-Type: application/json' -H "Authorization: Bearer $MCP_TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"<tool>","arguments":{}}}'
```

## Pitfalls
- For stateful or slow-starting MCP servers, detach a local proxy per `how-to-background.md` and close inherited side channels (`4>&- 5>&- 6>&-`).
- Never dump giant `tools/list` schemas raw into `msh` output; filter tool names/params with `jq`.
- Save binary/image MCP results to `/tmp/` and inspect via `aview` (or text tools if `vision=false`).
