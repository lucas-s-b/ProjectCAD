#!/usr/bin/env bash
# End-to-end test of the interference and clearance check between placed components, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase5d.out
rm -f "$OUT" target/mcp_phase5d_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase5d","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. gear 1, bolt 2 and nut 3 with default values, all at the origin
  cw 3 Model
  cw 4 Gear
  cw 5 "Add part"
  cw 6 Bolt
  cw 7 "Add part"
  cw 8 Nut
  cw 9 "Add part"
  call 10 state '{}'
  # 2. gear #1 at 0; bolt #1 head on the gear top (z 10), shank through the bore; nut #1 under the gear on the shank; nut #2 at x 16
  cw 11 Assembly
  cw 12 "Component body"
  cw 13 "Body 1: Gear 1"
  cw 14 "Add component"
  cw 15 "Component body"
  cw 16 "Body 2: Bolt 2"
  cw 17 "Add component"
  st 18 "Component Z" "10 mm"
  cw 19 "Component body"
  cw 20 "Body 3: Nut 3"
  cw 21 "Add component"
  st 22 "Component Z" "-6.8 mm"
  cw 23 "Add component"
  st 24 "Component X" "16 mm"
  st 25 "Component Z" "-6.8 mm"
  call 26 state '{}'
  # 3. check: four contacts, nut #1 to nut #2 clearance 3
  cw 27 Check
  call 28 state '{}'
  # 4. nut #1 0.8 mm into the gear: the edit drops the result; a new check finds the interference
  cw 29 "Nut 3 #1"
  st 30 "Component Z" "-6 mm"
  call 31 state '{}'
  cw 32 Check
  call 33 state '{}'
  cw 34 3D
  call 35 key '{"key":"F"}'
  call 36 screenshot '{}'
  # 5. a bad clearance limit is refused; 6 mm also lists bolt to nut #2
  st 37 "Clearance limit" "1 deg"
  cw 38 Check
  call 39 state '{}'
  st 40 "Clearance limit" "6 mm"
  cw 41 Check
  call 42 state '{}'
  # 6. undoing the nut move drops the result again
  call 43 key '{"key":"Z","ctrl":true}'
  call 44 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase5d.err

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
# x of the first and of the second closest point of that clearance
px1() { clear "$1" "$2" "$3" | grep -oE '"points":\[\[[-0-9.e]+' | grep -oE '[-0-9.e]+$'; }
px2() { clear "$1" "$2" "$3" | grep -oE '\],\[[-0-9.e]+' | grep -oE '[-0-9.e]+$'; }

[ "$(grep -c . "$OUT")" -ge 43 ] || bad "expected at least 43 responses, got $(grep -c . "$OUT")"
grep "\"id\":36," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase5d_36.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase5d_36.png | tail -c 3)" = "PNG" ] || bad "screenshot 36 is not a PNG"

# 1. three bodies
[ "$(nbodies 10)" -eq 3 ] || bad "expected 3 bodies, got $(nbodies 10)"
# 2. four components, nothing checked yet
[ "$(ncomps 26)" -eq 4 ] || bad "expected 4 components, got $(ncomps 26)"
flat 26 | grep -qF '"check":null' || bad "check is not null in state 26"
near "$(cmin 26 2 3)" -20 0.001 || bad "bolt min z is $(cmin 26 2 3), expected -20"
near "$(cmin 26 3 3)" -6.8 0.001 || bad "nut #1 min z is $(cmin 26 3 3), expected -6.8"
near "$(cmin 26 4 1)" 9.5 0.001 || bad "nut #2 min x is $(cmin 26 4 1), expected 9.5"
# 3. four contacts, one clearance
flat 28 | grep -qF '"status":"Check:interference0,contact4,clearance1"' || bad "status of state 28"
[ "$(nmeet 28)" -eq 4 ] || bad "expected 4 meetings, got $(nmeet 28)"
for pair in "1 2" "1 3" "1 4" "2 3"; do
  meeting 28 $pair | grep -qF '"error":null,"kind":"contact","volume":null}' || bad "meeting $pair of state 28 is not a contact"
done
[ "$(nclear 28)" -eq 1 ] || bad "expected 1 clearance, got $(nclear 28)"
near "$(clear 28 3 4 | val distance)" 3 0.0001 || bad "nut to nut clearance is $(clear 28 3 4 | val distance), expected 3"
near "$(px1 28 3 4)" 6.5 0.0001 || bad "closest point on nut #1 is x $(px1 28 3 4), expected 6.5"
near "$(px2 28 3 4)" 9.5 0.0001 || bad "closest point on nut #2 is x $(px2 28 3 4), expected 9.5"
# 4. an edit drops the result; the new check finds the interference
flat 31 | grep -qF '"check":null' || bad "check is not null in state 31"
flat 33 | grep -qF '"status":"Check:interference1,contact3,clearance1"' || bad "status of state 33"
meeting 33 1 3 | grep -qF '"error":null,"kind":"interference"' || bad "gear to nut #1 is not an interference"
near "$(meeting 33 1 3 | val volume)" 77.0 0.3 || bad "interference volume is $(meeting 33 1 3 | val volume), expected 77.0"
meeting 33 2 3 | grep -qF '"kind":"contact"' || bad "bolt to nut #1 is not a contact"
near "$(clear 33 3 4 | val distance)" 3 0.0001 || bad "nut to nut clearance of state 33 is $(clear 33 3 4 | val distance), expected 3"
# 5. a bad limit is refused; 6 mm lists more
flat 39 | grep -qF '"status":"clearancelimit:expectedalength"' || bad "status of state 39"
flat 39 | grep -qF '"check":null' || bad "check is not null in state 39"
[ "$(nclear 42)" -eq 2 ] || bad "expected 2 clearances, got $(nclear 42)"
near "$(clear 42 2 4 | val distance)" 5.5 0.025 || bad "bolt to nut #2 clearance is $(clear 42 2 4 | val distance), expected 5.5"
flat 42 | grep -oE '"clearances":\[\{"a":[0-9]+,"b":[0-9]+' | grep -qF '"a":3,"b":4' || bad "clearances of state 42 are not nearest first"
near "$(meeting 42 1 3 | val volume)" 77.0 0.3 || bad "interference volume of state 42 is $(meeting 42 1 3 | val volume), expected 77.0"
# 6. undo drops the result
flat 44 | grep -qF '"check":null' || bad "check is not null in state 44"
near "$(cmin 44 3 3)" -6.8 0.001 || bad "nut #1 min z after undo is $(cmin 44 3 3), expected -6.8"

[ "$fail" -eq 0 ] && echo "MCP PHASE5D PASS" || { echo "outputs in $OUT"; exit 1; }
