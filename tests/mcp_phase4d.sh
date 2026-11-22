#!/usr/bin/env bash
# End-to-end test of roofs, stairs and multi-floor views, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase4d.out
rm -f "$OUT" target/mcp_phase4d_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
wc_() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true${4:+,\"shift\":true}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase4d","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. level 1 (height 25 mm): walls 1-4 around (0,0)-(40,30), 2 thick
  cw 3 Model
  st 4 "Level height" "25 mm"
  cw 5 "Add level"
  cw 6 Wall
  st 7 "Wall thickness" 2
  wc_ 8 0 0
  wc_ 9 40 0
  wc_ 10 40 30
  wc_ 11 0 30
  wc_ 12 0 0
  call 13 key '{"key":"Escape"}'
  # 2. stair 5 from (5,5) towards +y, 4 wide: risers of at most 2.5 -> 10 for 25, treads 2 -> b (5,23)
  cw 14 Stair
  st 15 "Stair width" 4
  st 16 "Max riser" 2.5
  st 17 "Tread" 2
  wc_ 18 5 5
  wc_ 19 5 10
  call 20 key '{"key":"Escape"}'
  call 21 state '{}'
  # 3. level 2 at 25 (height 20): walls 6-9, room 10, gable roof 11 from the room (45 degrees, 1 thick, overhang 1)
  cw 22 Model
  st 23 "Level elevation" "25 mm"
  st 24 "Level height" "20 mm"
  cw 25 "Add level"
  cw 26 Wall
  wc_ 27 0 0
  wc_ 28 40 0
  wc_ 29 40 30
  wc_ 30 0 30
  wc_ 31 0 0
  call 32 key '{"key":"Escape"}'
  cw 33 Room
  wc_ 34 20 15
  call 35 key '{"key":"Escape"}'
  cw 36 Sketch
  wc_ 37 20 15
  cw 38 Gable
  st 39 "Roof pitch" 45
  st 40 "Roof thickness" 1
  st 41 "Roof overhang" 1
  cw 42 "Make roof"
  call 43 state '{}'
  # 4. level 1 in grey under level 2
  cw 44 Model
  cw 45 "Level below"
  call 46 screenshot '{}'
  # 5. level 1 height 30: its walls and stair follow; one undo restores them
  cw 47 "Level 1"
  st 48 "Level elevation" "0 mm"
  st 49 "Level height" "30 mm"
  cw 50 "Apply to level"
  call 51 state '{}'
  call 52 key '{"key":"Z","ctrl":true}'
  call 53 state '{}'
  cw 54 3D
  call 55 key '{"key":"F"}'
  call 56 screenshot '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase4d.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# the walls array of the model: entries {"id":..,"volume":..,"zmax":..,"zmin":..}
walls() { flat "$1" | grep -oE '"walls":\[[^]]*\]'; }
nwalls() { walls "$1" | grep -oE '"volume"' | wc -l; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# the rooms array of the model: entries {"area":..,"id":..,"perimeter":..}
rooms() { flat "$1" | grep -oE '"rooms":\[[^]]*\]'; }
room() { rooms "$1" | grep -oE "\{\"area\":[-0-9.e+]+,\"id\":$2,[^}]*\}"; }
wall() { walls "$1" | grep -oE "\{\"id\":$2,[^}]*\}"; }
# value of key $1 in the entry on stdin
val() { grep -oE "\"$1\":[-0-9.e+]+" | cut -d: -f2; }

[ "$(grep -c . "$OUT")" -ge 55 ] || bad "expected at least 55 responses, got $(grep -c . "$OUT")"
for n in 46 56; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase4d_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase4d_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. the stair: 10 risers of 2.5, b at (5,23)
[ "$(nwalls 21)" -eq 5 ] || bad "expected 5 solids, got $(nwalls 21)"
flat 21 | grep -qF '"risers":10' || bad "stair has not 10 risers"
# clicks land on whole pixels, so b is (5,23) only within a few micrometres
stair_b=$(flat 21 | grep -oE '"Stair":\{[^}]*\}' | grep -oE '"b":\[[-0-9.e+]+,[-0-9.e+]+\]' | sed 's/"b"://' | tr -d '[]' | tr ',' '\n')
near "$(echo "$stair_b" | sed -n 1p)" 5 1e-4 && near "$(echo "$stair_b" | sed -n 2p)" 23 1e-4 || bad "stair b is not (5,23): $stair_b"
near "$(wall 21 5 | val volume)" 900 1e-3 || bad "stair volume is $(wall 21 5 | val volume), expected 900"
near "$(wall 21 5 | val zmin)" 0 1e-6 || bad "stair zmin is $(wall 21 5 | val zmin), expected 0"
near "$(wall 21 5 | val zmax)" 22.5 1e-6 || bad "stair zmax is $(wall 21 5 | val zmax), expected 22.5"
# 2. level 2: walls 6-9, room 10, gable roof 11
[ "$(nwalls 43)" -eq 10 ] || bad "expected 10 solids, got $(nwalls 43)"
near "$(room 43 10 | val area)" 1064 1e-3 || bad "room 10 area is $(room 43 10 | val area), expected 1064"
flat 43 | grep -qF '"kind":"Gable"' || bad "no gable roof"
near "$(wall 43 6 | val zmin)" 25 1e-6 || bad "wall 6 zmin is $(wall 43 6 | val zmin), expected 25"
near "$(wall 43 6 | val zmax)" 45 1e-6 || bad "wall 6 zmax is $(wall 43 6 | val zmax), expected 45"
near "$(wall 43 11 | val volume)" 1697.056 1e-2 || bad "roof volume is $(wall 43 11 | val volume), expected 1697.056"
near "$(wall 43 11 | val zmin)" 44 1e-3 || bad "roof zmin is $(wall 43 11 | val zmin), expected 44"
near "$(wall 43 11 | val zmax)" 60.414 1e-2 || bad "roof zmax is $(wall 43 11 | val zmax), expected 60.414"
# 3. level 1 taller: walls and stair follow, level 2 does not
near "$(wall 51 1 | val zmax)" 30 1e-6 || bad "wall 1 zmax is $(wall 51 1 | val zmax), expected 30"
near "$(wall 51 6 | val zmax)" 45 1e-6 || bad "wall 6 zmax is $(wall 51 6 | val zmax), expected 45"
near "$(wall 51 5 | val zmax)" 27 1e-6 || bad "stair zmax is $(wall 51 5 | val zmax), expected 27"
near "$(wall 51 5 | val volume)" 1080 1e-3 || bad "stair volume is $(wall 51 5 | val volume), expected 1080"
flat 51 | grep -qF '"risers":10' || bad "stair lost its risers"
# 4. one undo restores the heights
near "$(wall 53 1 | val zmax)" 25 1e-6 || bad "wall 1 zmax after undo is $(wall 53 1 | val zmax), expected 25"
near "$(wall 53 5 | val zmax)" 22.5 1e-6 || bad "stair zmax after undo is $(wall 53 5 | val zmax), expected 22.5"

[ "$fail" -eq 0 ] && echo "MCP PHASE4D PASS" || { echo "outputs in $OUT"; exit 1; }
