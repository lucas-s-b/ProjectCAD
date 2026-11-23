#!/usr/bin/env bash
# End-to-end test of instanced drawing of a 100-component assembly, through `cad-app --mcp` only. No OS input.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase7a.out
ASM=target/mcp_phase7a.cadproj
rm -f "$OUT" "$ASM" target/mcp_phase7a.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

target/debug/cad-bench.exe assembly "$ASM" 100 || { echo "cannot write $ASM"; exit 1; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase7a","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. the assembly: 100 components of one gear
  call 3 open_project "{\"path\":\"$ASM\"}"
  cw 4 3D
  call 5 state '{}'
  # 2. select two components in turn: each selection is one full scene build
  cw 6 Assembly
  cw 7 "Gear #7"
  call 8 state '{}'
  cw 9 "Gear #8"
  call 10 state '{}'
  call 11 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase7a.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
# $1 >= $2
ge() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a >= b)}'; }
# the perf object of state $1
pf() { flat "$1" | grep -oE '"perf":\{[^}]*\}'; }
# placed components in state $1 (component rows sort error, id, max)
placed() { flat "$1" | grep -oE '"error":null,"id":[0-9]+,"max"' | wc -l; }

[ "$(grep -c . "$OUT")" -ge 10 ] || bad "expected at least 10 responses, got $(grep -c . "$OUT")"

# 5. all components placed
[ "$(placed 5)" -eq 100 ] || bad "placed components of state 5: $(placed 5)"
# 8 and 10. a scene build after each selection
for i in 8 10; do
  ge "$(pf $i | num scene_ms)" 0 || bad "scene_ms of state $i: $(pf $i)"
  ge "$(pf $i | num scene_vertices)" 1 || bad "scene_vertices of state $i: $(pf $i)"
done
# 11. the picture
grep "\"id\":11," "$OUT" | grep -qF '"mimeType":"image/png"' || bad "no png in response 11"
grep "\"id\":11," "$OUT" | grep -oE '"data":"[^"]*"' | head -1 | cut -d'"' -f4 | base64 -d > target/mcp_phase7a.png
[ -s target/mcp_phase7a.png ] || bad "target/mcp_phase7a.png is empty"

# Instancing: one mesh (the gear), one instance per component; the scene buffers hold only the grid and axes.
for i in 8 10; do
  [ "$(pf $i | num instances)" = 100 ] || bad "instances of state $i: $(pf $i)"
  [ "$(pf $i | num meshes)" = 1 ] || bad "meshes of state $i: $(pf $i)"
  [ "$(pf $i | num scene_vertices)" -lt 2000 ] || bad "scene_vertices of state $i: $(pf $i)"
done
[ "$(pf 5 | num instances)" = 100 ] || bad "instances of state 5: $(pf 5)"

echo "perf 8: $(pf 8)"; echo "perf 10: $(pf 10)"
[ "$fail" -eq 0 ] && echo "MCP PHASE7A PASS" || { echo "outputs in $OUT"; exit 1; }
