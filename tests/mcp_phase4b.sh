#!/usr/bin/env bash
# End-to-end test of doors and windows in walls, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase4b.out
rm -f "$OUT" target/mcp_phase4b_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
wc_() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true${4:+,\"shift\":true}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase4b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. level (height 25 mm), wall 1 (0,0)-(40,0), 2 thick
  cw 3 Model
  st 4 "Level height" "25 mm"
  cw 5 "Add level"
  cw 6 Wall
  st 7 "Wall thickness" 2
  wc_ 8 0 0
  wc_ 9 40 0
  call 10 key '{"key":"Escape"}'
  # 2. door 2 (8 x 20) at x 10, window 3 (10 x 10 on a sill of 8) at x 30
  cw 11 Door
  st 12 "Opening width" 8
  st 13 "Opening height" 20
  wc_ 14 10 0
  cw 15 Window
  st 16 "Opening width" 10
  st 17 "Opening height" 10
  st 18 "Opening sill" 8
  wc_ 19 30 0
  call 20 key '{"key":"Escape"}'
  call 21 state '{}'
  # 3. the wall moved to y 10 and shortened to (0,10)-(30,10) in Properties: the openings follow, the window slides in
  cw 22 Sketch
  wc_ 23 2 0
  st 24 "a y" 10
  st 25 "b y" 10
  st 26 "b x" 30
  call 27 state '{}'
  # 4. door made 6 wide and swung right (the click hits door and wall: the door wins)
  wc_ 28 10 10
  st 29 width 6
  cw 30 "swings left"
  call 31 state '{}'
  call 32 screenshot '{}'
  # 5. deleting the wall deletes its openings; undo brings all three back
  wc_ 33 2 10
  call 34 key '{"key":"Delete"}'
  call 35 state '{}'
  call 36 key '{"key":"Z","ctrl":true}'
  call 37 state '{}'
  cw 38 3D
  call 39 key '{"key":"F"}'
  call 40 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase4b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# the walls array of the model: entries {"id":..,"volume":..,"zmax":..,"zmin":..}
walls() { flat "$1" | grep -oE '"walls":\[[^]]*\]'; }
wvol() { walls "$1" | grep -oE '"volume":[-0-9.e+]+' | cut -d: -f2 | awk '{s += $1} END {print s}'; }
nwalls() { walls "$1" | grep -oE '"volume"' | wc -l; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }

[ "$(grep -c . "$OUT")" -ge 39 ] || bad "expected at least 39 responses, got $(grep -c . "$OUT")"
for n in 32 40; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase4b_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase4b_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. wall 40 x 2 x 25 = 2000, door 8 x 2 x 20 = 320, window 10 x 2 x 10 = 200
[ "$(nwalls 21)" -eq 1 ] || bad "expected 1 wall after step 2, got $(nwalls 21)"
near "$(wvol 21)" 1480 1e-3 || bad "wall volume after step 2 is $(wvol 21), expected 1480"
flat 21 | grep -qF '"hinge_b":false,"host":1,"kind":"Door","sill":0.0,"swing_left":true' || bad "door fields missing in the state"
flat 21 | grep -qF '"host":1,"kind":"Window","sill":8.0' || bad "window fields missing in the state"
# 2. 30 x 2 x 25 = 1500 - 320 - 200: the window, centred at 30, slid to 20..30
near "$(wvol 27)" 980 1e-3 || bad "wall volume after the wall edit is $(wvol 27), expected 980"
[ "$(flat 27 | grep -oF '"b":[30.0,10.0],"height":25.0,"thickness":2.0},"width"' | wc -l)" -eq 2 ] || bad "wall copies of the openings are not refreshed"
# 3. 1500 - 6 x 2 x 20 - 200
near "$(wvol 31)" 1060 1e-3 || bad "wall volume after the door edit is $(wvol 31), expected 1060"
flat 31 | grep -qF '"swing_left":false' || bad "door swing is not right"
flat 31 | grep -qF '"width":6.0' || bad "door width is not 6"
# 4. delete takes the openings with the wall
[ "$(nwalls 35)" -eq 0 ] || bad "walls left after delete: $(nwalls 35)"
! flat 35 | grep -qF '"Opening"' || bad "openings left after deleting their wall"
# 5. undo brings the wall and both openings back
near "$(wvol 37)" 1060 1e-3 || bad "wall volume after undo is $(wvol 37), expected 1060"
[ "$(flat 37 | grep -oF '"Opening"' | wc -l)" -eq 2 ] || bad "expected 2 openings after undo"

[ "$fail" -eq 0 ] && echo "MCP PHASE4B PASS" || { echo "outputs in $OUT"; exit 1; }
