#!/usr/bin/env bash
# End-to-end test of the part generators (gear, shaft, spring, bolt, nut), through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase5a.out
SAVED=target/mcp_phase5a.cadproj
rm -f "$OUT" "$SAVED" target/mcp_phase5a_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase5a","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. gear 1 with the default values (m 2, z 20, 20 deg, b 10, bore 8)
  cw 3 Model
  cw 4 Gear
  cw 5 "Add part"
  call 6 state '{}'
  # 2. parameter teeth = 24 drives the gear
  cw 7 "Param name"
  call 8 type_text '{"text":"teeth"}'
  cw 9 "Param expression"
  call 10 type_text '{"text":"24"}'
  cw 11 Add
  st 12 "Gear teeth" teeth
  call 13 state '{}'
  # 3. bolt 2: M10 x 40 at x 60, with its designation
  cw 14 Bolt
  cw 15 "Add part"
  st 16 "Thread size" M10
  st 17 "Bolt length" "40 mm"
  st 18 "Part X" "60 mm"
  call 19 widgets '{}'
  call 20 state '{}'
  # 4. nut 3 at x 120, spring 4 at y 60, shaft 5 at x 180 (default values)
  cw 21 Nut
  cw 22 "Add part"
  st 23 "Part X" "120 mm"
  cw 24 Spring
  cw 25 "Add part"
  st 26 "Part Y" "60 mm"
  cw 27 Shaft
  cw 28 "Add part"
  st 29 "Part X" "180 mm"
  call 30 state '{}'
  # 5. teeth = 30
  st 31 "Param teeth" 30
  call 32 state '{}'
  # 6. save, new project, open: the same parts come back
  call 33 save_project "{\"path\":\"$SAVED\"}"
  call 34 new_project '{}'
  call 35 open_project "{\"path\":\"$SAVED\"}"
  call 36 state '{}'
  cw 37 3D
  call 38 key '{"key":"F"}'
  call 39 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase5a.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# value of key $1 in the entry on stdin
val() { grep -oE "\"$1\":[-0-9.e+]+" | cut -d: -f2; }
# the bodies array of the model: entries {"edges":..,"id":..,"volume":..}
bodies() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
nbodies() { bodies "$1" | grep -oE '"volume"' | wc -l; }
body() { bodies "$1" | grep -oE "\{\"edges\":[0-9]+,\"id\":$2,[^}]*\}"; }

[ "$(grep -c . "$OUT")" -ge 38 ] || bad "expected at least 38 responses, got $(grep -c . "$OUT")"
grep "\"id\":39," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase5a_39.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase5a_39.png | tail -c 3)" = "PNG" ] || bad "screenshot 39 is not a PNG"

# 1. gear 1 with the default values (1175.514 x 10 plus about 3.2 that the 32-gon bore leaves at 0.02 mm)
[ "$(nbodies 6)" -eq 1 ] || bad "expected 1 body, got $(nbodies 6)"
near "$(body 6 1 | val volume)" 11758 30 || bad "gear volume is $(body 6 1 | val volume), expected 11758"
# 2. teeth = 24
near "$(body 13 1 | val volume)" 17250 30 || bad "gear volume with 24 teeth is $(body 13 1 | val volume), expected 17250"
flat 13 | grep -qF '"teeth":"teeth"' || bad "gear teeth are not the parameter"
# 3. the bolt and its designation
grep "\"id\":19," "$OUT" | grep -qF 'Designation: ISO 4017 M10x40' || bad "no designation for the M10x40 bolt"
[ "$(nbodies 20)" -eq 2 ] || bad "expected 2 bodies, got $(nbodies 20)"
# hexagon (sqrt 3 / 2) x 16^2 x 6.4 = 1418.9 plus a 36-gon shank 78.14 x 40 = 3125.7
near "$(body 20 2 | val volume)" 4545 25 || bad "bolt volume is $(body 20 2 | val volume), expected 4545"
flat 20 | grep -qF '"size":"M10"' || bad "bolt size is not M10"
flat 20 | grep -qF '"length":"40mm"' || bad "bolt length is not 40 mm"
# 4. five parts, all built
[ "$(nbodies 30)" -eq 5 ] || bad "expected 5 bodies, got $(nbodies 30)"
near "$(body 30 3 | val volume)" 655.6 3 || bad "nut volume is $(body 30 3 | val volume), expected 655.6"
near "$(body 30 4 | val volume)" 1173.738 0.01 || bad "spring volume is $(body 30 4 | val volume), expected 1173.738"
near "$(body 30 5 | val volume)" 19372.34 0.01 || bad "shaft volume is $(body 30 5 | val volume), expected 19372.34"
! flat 30 | grep -q 'Error:' || bad "a feature is in error"
# 5. teeth = 30
near "$(body 32 1 | val volume)" 27387 30 || bad "gear volume with 30 teeth is $(body 32 1 | val volume), expected 27387"
# 6. the saved file comes back the same
[ "$(nbodies 36)" -eq 5 ] || bad "expected 5 bodies after open, got $(nbodies 36)"
near "$(body 36 1 | val volume)" 27387 30 || bad "gear volume after open is $(body 36 1 | val volume), expected 27387"
near "$(body 36 5 | val volume)" 19372.34 0.01 || bad "shaft volume after open is $(body 36 5 | val volume), expected 19372.34"

[ "$fail" -eq 0 ] && echo "MCP PHASE5A PASS" || { echo "outputs in $OUT"; exit 1; }
