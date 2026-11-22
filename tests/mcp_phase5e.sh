#!/usr/bin/env bash
# End-to-end test of bearings and modelled threads, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase5e.out
rm -f "$OUT" target/mcp_phase5e_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase5e","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. bearing 1 (6204), shaft 2, bolt 3 and nut 4 with modelled threads, all at the origin
  cw 3 Model
  cw 4 Bearing
  cw 5 "Add part"
  call 6 widgets '{}'
  cw 7 Shaft
  cw 8 "Add part"
  cw 9 Bolt
  cw 10 "Add part"
  cw 11 "Modelled thread"
  cw 12 Nut
  cw 13 "Add part"
  cw 14 "Modelled thread"
  call 15 state '{}'
  # 2. shaft #1 at 0, bearing #1 on it at z 10; bolt #1 at x 60; nut #1 8 pitches below its bearing face, nut #2 16.5 pitches
  cw 16 Assembly
  cw 17 "Component body"
  cw 18 "Body 2: Shaft 2"
  cw 19 "Add component"
  cw 20 "Component body"
  cw 21 "Body 1: Bearing 1"
  cw 22 "Add component"
  st 23 "Component Z" "10 mm"
  cw 24 "Component body"
  cw 25 "Body 3: Bolt 3"
  cw 26 "Add component"
  st 27 "Component X" "60 mm"
  cw 28 "Component body"
  cw 29 "Body 4: Nut 4"
  cw 30 "Add component"
  st 31 "Component X" "60 mm"
  st 32 "Component Z" "-10 mm"
  cw 33 "Add component"
  st 34 "Component X" "60 mm"
  st 35 "Component Z" "-20.625 mm"
  call 36 state '{}'
  # 3. check: shaft in bearing is contact, nut #1 fits (clearance 0.02), nut #2 is half a pitch off (interference)
  cw 37 Check
  call 38 state '{}'
  cw 39 3D
  call 40 key '{"key":"F"}'
  call 41 screenshot '{}'
  # 4. nut #2 turned half a turn: its thread fits too
  st 42 "Rotation Z" "180 deg"
  call 43 state '{}'
  cw 44 Check
  call 45 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase5e.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# $1 lies in the closed range $2..$3
within() { awk -v a="$1" -v lo="$2" -v hi="$3" 'BEGIN {exit !(a != "" && a >= lo && a <= hi)}'; }
# value of key $1 in the entry on stdin
val() { grep -oE "\"$1\":[-0-9.e+]+" | cut -d: -f2; }
# the bodies array of the model: entries {"edges":..,"id":..,"volume":..}
bodies() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
nbodies() { bodies "$1" | grep -oE '"volume"' | wc -l; }
# volume of body $2 in the bodies array: {"edges":..,"id":$2,"volume":..}
bvol() { bodies "$1" | grep -oE "\"id\":$2,\"volume\":[-0-9.e]+" | cut -d: -f3; }
# the entry of component $2 in the state's components array: {"body":..,"error":..,"id":..,"max":[..],"min":[..],"name":..,"volume":..}
comp() { flat "$1" | grep -oE "\{\"body\":[0-9]+,\"error\":(null|\"[^\"]*\"),\"id\":$2,\"max\":\[[^]]*\],\"min\":\[[^]]*\],\"name\":\"[^\"]*\",\"volume\":[-0-9.e]+\}"; }
ncomps() { flat "$1" | grep -oE '\{"body":[0-9]+,"error":' | wc -l; }
# coordinate $3 (1 x, 2 y, 3 z) of the min corner of component $2
cmin() { comp "$1" "$2" | grep -oE '"min":\[[^]]*\]' | sed 's/"min":\[//; s/\]//' | cut -d, -f"$3"; }
# the check's meeting of components $2 and $3: {"a":..,"b":..,"error":..,"kind":"..","volume":..}
meeting() { flat "$1" | grep -oE "\{\"a\":$2,\"b\":$3,\"error\":(null|\"[^\"]*\"),\"kind\":\"[a-z]+\",\"volume\":(null|[-0-9.e]+)\}"; }
nmeet() { flat "$1" | grep -oE '\{"a":[0-9]+,"b":[0-9]+,"error":' | wc -l; }
# the check's clearance of components $2 and $3: {"a":..,"b":..,"distance":..,"points":[[x,y,z],[x,y,z]]}
clear() { flat "$1" | grep -oE "\{\"a\":$2,\"b\":$3,\"distance\":[-0-9.e]+,\"points\":(null|\[\[[^]]*\],\[[^]]*\]\])\}"; }
nclear() { flat "$1" | grep -oE '\{"a":[0-9]+,"b":[0-9]+,"distance":' | wc -l; }

[ "$(grep -c . "$OUT")" -ge 44 ] || bad "expected at least 44 responses, got $(grep -c . "$OUT")"
grep "\"id\":41," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase5e_41.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase5e_41.png | tail -c 3)" = "PNG" ] || bad "screenshot 41 is not a PNG"

# 1. four bodies; the bearing has its designation
grep "\"id\":6," "$OUT" | grep -qF 'Designation: Ball bearing 6204 20x47x14' || bad "no bearing designation in widgets 6"
[ "$(nbodies 15)" -eq 4 ] || bad "expected 4 bodies, got $(nbodies 15)"
near "$(bvol 15 1)" 13146.7 132 || bad "bearing volume is $(bvol 15 1), expected 13146.7"
near "$(bvol 15 2)" 19372.34 0.01 || bad "shaft volume is $(bvol 15 2), expected 19372.34"
near "$(bvol 15 3)" 1985.867 0.01 || bad "modelled bolt volume is $(bvol 15 3), expected 1985.867"
near "$(bvol 15 4)" 714.798 0.01 || bad "modelled nut volume is $(bvol 15 4), expected 714.798"
# 2. five components, nothing checked yet
[ "$(ncomps 36)" -eq 5 ] || bad "expected 5 components, got $(ncomps 36)"
flat 36 | grep -qF '"check":null' || bad "check is not null in state 36"
near "$(cmin 36 2 3)" 10 0.001 || bad "bearing min z is $(cmin 36 2 3), expected 10"
near "$(cmin 36 3 3)" -30 0.001 || bad "bolt min z is $(cmin 36 3 3), expected -30"
near "$(cmin 36 3 1)" 53.5 0.001 || bad "bolt min x is $(cmin 36 3 1), expected 53.5"
near "$(cmin 36 4 3)" -10 0.001 || bad "nut #1 min z is $(cmin 36 4 3), expected -10"
near "$(cmin 36 5 3)" -20.625 0.001 || bad "nut #2 min z is $(cmin 36 5 3), expected -20.625"
# 3. shaft in bearing and nut #1 on the bolt fit, nut #2 is half a pitch off
flat 38 | grep -qF '"status":"Check:interference1,contact1,clearance2"' || bad "status of state 38"
[ "$(nmeet 38)" -eq 2 ] || bad "expected 2 meetings, got $(nmeet 38)"
meeting 38 1 2 | grep -qF '"error":null,"kind":"contact","volume":null}' || bad "shaft to bearing is not a contact"
meeting 38 3 5 | grep -qF '"error":null,"kind":"interference"' || bad "bolt to nut #2 is not an interference"
within "$(meeting 38 3 5 | val volume)" 25 40 || bad "flank overlap is $(meeting 38 3 5 | val volume), expected 25..40"
[ "$(nclear 38)" -eq 2 ] || bad "expected 2 clearances, got $(nclear 38)"
within "$(clear 38 3 4 | val distance)" 0.0185 0.0205 || bad "bolt to nut #1 clearance is $(clear 38 3 4 | val distance), expected 0.0185..0.0205"
near "$(clear 38 4 5 | val distance)" 3.825 0.0001 || bad "nut to nut clearance is $(clear 38 4 5 | val distance), expected 3.825"
flat 38 | grep -oE '"clearances":\[\{"a":[0-9]+,"b":[0-9]+' | grep -qF '"a":3,"b":4' || bad "clearances of state 38 are not nearest first"
# 4. nut #2 turned half a turn: its thread fits too
flat 43 | grep -qF '"check":null' || bad "check is not null in state 43"
flat 45 | grep -qF '"status":"Check:interference0,contact1,clearance3"' || bad "status of state 45"
[ "$(nclear 45)" -eq 3 ] || bad "expected 3 clearances, got $(nclear 45)"
within "$(clear 45 3 5 | val distance)" 0.0185 0.0205 || bad "bolt to nut #2 clearance after the turn is $(clear 45 3 5 | val distance), expected 0.0185..0.0205"
near "$(clear 45 4 5 | val distance)" 3.825 0.0001 || bad "nut to nut clearance of state 45 is $(clear 45 4 5 | val distance), expected 3.825"

[ "$fail" -eq 0 ] && echo "MCP PHASE5E PASS" || { echo "outputs in $OUT"; exit 1; }
