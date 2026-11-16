#!/usr/bin/env bash
# End-to-end test of `cad-app --mcp` over its own stdin/stdout. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_smoke.out
SAVED=target/mcp_smoke.cadproj
rm -f "$OUT" "$SAVED"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
  cw 3 Line
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":0,"world":true}'
  call 6 state '{}'
  call 7 key '{"key":"Z","ctrl":true}'
  call 8 state '{}'
  call 9 key '{"key":"Y","ctrl":true}'
  call 10 save_project "{\"path\":\"$SAVED\"}"
  call 11 screenshot '{}'
  call 12 click '{"x":10}'
  call 13 widgets '{}'
  call 14 click_widget '{"name":"No such widget"}'
  call 15 set_text '{"name":"Coordinates","text":"5,5"}'
  call 16 state '{}'
  call 17 export_project '{"path":"target/mcp_smoke_none.stl"}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_smoke.err

fail=0
check() { # description, grep -E pattern, line id
  if ! grep -E "^\{\"id\":$3,|\"id\":$3," "$OUT" | grep -qE "$2"; then echo "FAIL: $1"; fail=1; fi
}
[ "$(grep -c . "$OUT")" -ge 16 ] || { echo "FAIL: expected 16 responses, got $(grep -c . "$OUT")"; fail=1; }
[ "$(grep '"id":2,' "$OUT" | grep -o '"inputSchema"' | wc -l)" -eq 15 ] || { echo "FAIL: 15 tools not listed"; fail=1; }
check 'Line tool selected' '\\"tool\\":\\"Line\\"' 3
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
flat 6 | grep -q '"Line":{' || { echo "FAIL: no Line entity after drawing"; fail=1; }
flat 6 | grep -qE '"entities":1' || { echo "FAIL: expected 1 entity after drawing"; fail=1; }
flat 6 | grep -qE '"a":\[0(\.0)?,0(\.0)?\],"b":\[40(\.0)?,0(\.0)?\]' || { echo "FAIL: line (0,0)-(40,0) missing"; fail=1; }
flat 8 | grep -qE '"entities":0' || { echo "FAIL: undo left entities"; fail=1; }
[ -f "$SAVED" ] && grep -q '"Line"' "$SAVED" || { echo "FAIL: saved file missing or has no Line"; fail=1; }
check 'screenshot png' '"mimeType":"image/png"' 11
check 'missing y is an error' '"isError":true' 12
flat 13 | grep -q '"name":"Line"' || { echo "FAIL: widgets lacks Line"; fail=1; }
flat 13 | grep -q '"name":"Coordinates"' || { echo "FAIL: widgets lacks Coordinates"; fail=1; }
check 'unknown widget is an error' 'index 0[)] not found.*"isError":true' 14
check 'set_text reached the field' '"isError":false' 15
check 'STL export without a body is an error' 'no solid body to export.*"isError":true' 17
flat 16 | grep -q '"Line":{' || { echo "FAIL: line vanished after set_text"; fail=1; }
[ "$fail" -eq 0 ] && echo "MCP SMOKE PASS" || { echo "outputs in $OUT"; exit 1; }
