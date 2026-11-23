#!/usr/bin/env bash
# End-to-end test of the tetrahedral mesher for analysis (a box and the default shaft), through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase6d.out
rm -f "$OUT" target/mcp_phase6d_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase6d","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 20 x 10 x 10 (feature 1 "Extrude 1"); no mesh yet
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":20,"y":10,"world":true}'
  cw 6 Model
  call 7 click '{"x":10,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  call 10 state '{}'
  # 2. mesh it at 5 mm: exact
  st 11 "Element size" "5 mm"
  cw 12 "Mesh for analysis"
  call 13 state '{}'
  call 14 widgets '{}'
  # 3. shrunken elements in 3D
  cw 15 "Shrink elements"
  cw 16 3D
  call 17 key '{"key":"F"}'
  call 18 screenshot '{}'
  # 4. the default shaft as body 2 at x 100: the edit drops the mesh
  cw 19 Model
  cw 20 Shaft
  cw 21 "Add part"
  st 22 "Part X" "100 mm"
  call 23 state '{}'
  # 5. the shaft at 2 mm
  cw 24 "Mesh body"
  cw 25 "Body 2: Shaft 2"
  st 26 "Element size" "2 mm"
  cw 27 "Mesh for analysis"
  call 28 state '{}'
  # 6. too fine: refused
  st 29 "Element size" "0.1 mm"
  cw 30 "Mesh for analysis"
  call 31 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase6d.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
# the fea_mesh object of state $1 (it holds no nested braces)
fm() { flat "$1" | grep -oE '"fea_mesh":\{[^}]*\}'; }
# $1 >= $2
ge() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a >= b)}'; }

[ "$(grep -c . "$OUT")" -ge 30 ] || bad "expected at least 30 responses, got $(grep -c . "$OUT")"
grep "\"id\":18," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase6d_18.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase6d_18.png | tail -c 3)" = "PNG" ] || bad "screenshot 18 is not a PNG"

# 1. no mesh yet
flat 10 | grep -qF '"fea_mesh":null' || bad "fea_mesh of state 10 is not null"
# 2. the box at 5 mm
fm 13 | grep -qF '"accuracy":"approximate(mesh)","body":1,' || bad "fea_mesh of state 13: $(fm 13)"
[ "$(fm 13 | num nodes)" = 225 ] || bad "nodes of state 13 is $(fm 13 | num nodes), expected 225"
[ "$(fm 13 | num elements)" = 96 ] || bad "elements of state 13 is $(fm 13 | num elements), expected 96"
[ "$(fm 13 | num inverted)" = 0 ] || bad "inverted of state 13 is $(fm 13 | num inverted)"
[ "$(fm 13 | num unsnapped)" = 0 ] || bad "unsnapped of state 13 is $(fm 13 | num unsnapped)"
near "$(fm 13 | num volume)" 2000 0.000001 || bad "volume of state 13 is $(fm 13 | num volume)"
near "$(fm 13 | num body_volume)" 2000 0.000001 || bad "body_volume of state 13 is $(fm 13 | num body_volume)"
near "$(fm 13 | num volume_error)" 0 0.000000001 || bad "volume_error of state 13 is $(fm 13 | num volume_error)"
near "$(fm 13 | num min_ratio)" 0.717439 0.000001 || bad "min_ratio of state 13 is $(fm 13 | num min_ratio)"
near "$(fm 13 | num min_dihedral)" 45 0.000001 || bad "min_dihedral of state 13 is $(fm 13 | num min_dihedral)"
near "$(fm 13 | num h)" 5 0.000001 || bad "h of state 13 is $(fm 13 | num h)"
fm 13 | grep -qF '"error":null' || bad "error of state 13 is not null"
for t in 'Mesh for analysis' 'Shrink elements' 'approximate (mesh)'; do
  grep "\"id\":14," "$OUT" | grep -qF "$t" || bad "no '$t' in widgets 14"
done
# 4. an edit drops the mesh
flat 23 | grep -qF '"fea_mesh":null' || bad "fea_mesh of state 23 is not null"
# 5. the shaft at 2 mm
fm 28 | grep -qF '"body":2,' || bad "fea_mesh of state 28: $(fm 28)"
[ "$(fm 28 | num inverted)" = 0 ] || bad "inverted of state 28 is $(fm 28 | num inverted)"
ge "$(fm 28 | num min_ratio)" 0.15 || bad "min_ratio of state 28 is $(fm 28 | num min_ratio)"
ge "$(fm 28 | num elements)" 10000 || bad "elements of state 28 is $(fm 28 | num elements)"
near "$(fm 28 | num volume_error)" 0 0.01 || bad "volume_error of state 28 is $(fm 28 | num volume_error)"
near "$(fm 28 | num body_volume)" 19372.34 0.01 || bad "body_volume of state 28 is $(fm 28 | num body_volume)"
# 6. too fine
fm 31 | grep -qF '{"error":"elementsize0.1mmgives32000000gridcells(atmost50000);usealargerelementsize"}' || bad "fea_mesh of state 31: $(fm 31)"

[ "$fail" -eq 0 ] && echo "MCP PHASE6D PASS" || { echo "outputs in $OUT"; exit 1; }
