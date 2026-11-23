#!/usr/bin/env bash
# End-to-end test of the constraints panel through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_constraints.out
rm -f "$OUT" target/mcp_constraints_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"constraints","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  cw 3 Line
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":10,"world":true}'
  call 6 click '{"x":0,"y":30,"world":true}'
  call 7 click '{"x":40,"y":45,"world":true}'
  call 8 key '{"key":"Escape"}'
  call 9 click '{"x":20,"y":5,"world":true}'
  call 10 screenshot '{}'
  cw 11 Horizontal
  call 12 state '{}'
  call 13 key '{"key":"Z","ctrl":true}'
  call 14 state '{}'
  call 15 key '{"key":"Y","ctrl":true}'
  call 16 state '{}'
  call 17 click '{"x":20,"y":5,"world":true}'
  call 18 click '{"x":20,"y":37.5,"world":true,"shift":true}'
  call 19 screenshot '{}'
  cw 20 Parallel
  call 21 state '{}'
  call 22 click '{"x":20,"y":5,"world":true}'
  call 23 key '{"key":"Delete"}'
  call 24 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_constraints.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# "ax ay bx by" of line entity $2 in the state response $1
line_pts() {
  flat "$1" | grep -oE "\"id\":$2,\"layer\":[0-9]+,(\"meta\":\{[^}]*\},)?\"name\":\"[^\"]*\",\"shape\":\{\"Line\":\{\"a\":\[[^]]*\],\"b\":\[[^]]*\]" \
    | grep -oE '\[[^]]*\]' | tr -d '[]' | tr ',' ' ' | tr '\n' ' '
}
horizontal() { echo "$1" | awk '{d = $2 - $4; exit !(NF == 4 && d < 1e-6 && d > -1e-6)}'; }
n_constraints() { flat "$1" | sed -E 's/.*"constraints":(.*),"entities":\{.*/\1/' | grep -oE '"[0-9]+":\{"[A-Za-z]+"' | wc -l; }
n_entities() { flat "$1" | grep -oE '"entities":[0-9]+' | cut -d: -f2; }

[ "$(grep -c . "$OUT")" -ge 22 ] || bad "expected 22 responses, got $(grep -c . "$OUT")"
for id in 10 19; do
  grep "\"id\":$id," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "target/mcp_constraints_$id.png" 2>/dev/null
  [ "$(head -c 4 "target/mcp_constraints_$id.png" | tail -c 3)" = "PNG" ] || bad "screenshot $id is not a PNG"
done

# 3. Horizontal on line 1
p=$(line_pts 12 1)
horizontal "$p" || bad "line 1 not horizontal after the Horizontal button ($p)"
[ "$(n_constraints 12)" -eq 1 ] || bad "expected 1 constraint after Horizontal"
flat 12 | grep -q '"constraint_status":"Under-constrained' || bad "constraint_status is not Under-constrained after Horizontal"
# 4. Ctrl+Z and Ctrl+Y
p=$(line_pts 14 1)
horizontal "$p" && bad "line 1 still horizontal after undo ($p)"
echo "$p" | awk '{exit !($1 == 0 && $2 == 0 && $3 == 40 && $4 == 10)}' || bad "undo did not restore line 1 exactly ($p)"
[ "$(n_constraints 14)" -eq 0 ] || bad "constraints left after undo"
flat 14 | grep -q '"constraint_status":null' || bad "constraint_status not null after undo"
horizontal "$(line_pts 16 1)" || bad "line 1 not horizontal after redo"
[ "$(n_constraints 16)" -eq 1 ] || bad "expected 1 constraint after redo"
# 5. Parallel on both lines
horizontal "$(line_pts 21 2)" || bad "line 2 not horizontal after Parallel ($(line_pts 21 2))"
horizontal "$(line_pts 21 1)" || bad "line 1 not horizontal after Parallel"
flat 21 | grep -q '"Parallel"' || bad "no Parallel constraint"
[ "$(n_constraints 21)" -eq 2 ] || bad "expected 2 constraints after Parallel"
# 6. Delete line 1
[ "$(n_entities 24)" -eq 1 ] || bad "expected 1 entity after Delete"
[ -n "$(line_pts 24 2)" ] && [ -z "$(line_pts 24 1)" ] || bad "line 1 should be gone and line 2 should remain"
[ "$(n_constraints 24)" -eq 0 ] || bad "constraints left after deleting line 1"

[ "$fail" -eq 0 ] && echo "MCP CONSTRAINTS PASS" || { echo "outputs in $OUT"; exit 1; }
