#!/usr/bin/env bash
# End-to-end test of rooms and floors, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase4c.out
rm -f "$OUT" target/mcp_phase4c_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
wc_() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true${4:+,\"shift\":true}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase4c","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. level (height 25 mm); walls 1-4 around (0,0)-(40,30), 2 thick; partition wall 5 at x 20
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
  cw 14 Wall
  wc_ 15 20 0
  wc_ 16 20 30
  call 17 key '{"key":"Escape"}'
  # 2. door 6 (8 x 20) in the partition: it does not join the rooms
  cw 18 Door
  st 19 "Opening width" 8
  st 20 "Opening height" 20
  wc_ 21 20 15
  # 3. rooms 7 and 8, one click each; a click outside the walls adds nothing
  cw 22 Room
  wc_ 23 10 15
  wc_ 24 30 15
  wc_ 25 60 15
  call 26 key '{"key":"Escape"}'
  call 27 state '{}'
  # 4. name of room 7 and number of room 8 in the Rooms list (Model tab)
  st 28 "Room name" Kitchen 0
  st 29 "Room number" B2 1
  call 30 state '{}'
  call 31 screenshot '{}'
  # 5. partition moved to x 25 in Properties: the rooms follow, the names stay
  cw 32 Sketch
  wc_ 33 20 5
  st 34 "a x" 25
  st 35 "b x" 25
  call 36 state '{}'
  # 6. floor 9 from room 7, 3 thick
  wc_ 37 10 15
  st 38 "Floor thickness" 3
  cw 39 "Make floor"
  call 40 state '{}'
  # 7. rectangle 10 (-30,0)-(-10,20) becomes floor 11; undo brings the rectangle back
  cw 41 Rect
  wc_ 42 -30 0
  wc_ 43 -10 20
  call 44 key '{"key":"Escape"}'
  wc_ 45 -30 10
  cw 46 "Make floor"
  call 47 state '{}'
  call 48 key '{"key":"Z","ctrl":true}'
  call 49 state '{}'
  cw 50 3D
  call 51 key '{"key":"F"}'
  call 52 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase4c.err

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

[ "$(grep -c . "$OUT")" -ge 51 ] || bad "expected at least 51 responses, got $(grep -c . "$OUT")"
for n in 31 52; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase4c_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase4c_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. two rooms, x 1..19 and 21..39, y 1..29: 18 x 28 each; the click outside added nothing
[ "$(nwalls 27)" -eq 5 ] || bad "expected 5 walls, got $(nwalls 27)"
[ "$(flat 27 | grep -oF '"Room":{' | wc -l)" -eq 2 ] || bad "expected 2 rooms"
near "$(room 27 7 | val area)" 504 1e-3 || bad "room 7 area is $(room 27 7 | val area), expected 504"
near "$(room 27 8 | val area)" 504 1e-3 || bad "room 8 area is $(room 27 8 | val area), expected 504"
near "$(room 27 7 | val perimeter)" 92 1e-3 || bad "room 7 perimeter is $(room 27 7 | val perimeter), expected 92"
# 2. the Rooms list edits name and number
flat 30 | grep -qF '"name":"Kitchen","number":"1"' || bad "room 7 is not Kitchen 1"
flat 30 | grep -qF '"name":"Room","number":"B2"' || bad "room 8 is not Room B2"
# 3. partition at x 25: the rooms follow, the names stay
near "$(room 36 7 | val area)" 644 1e-3 || bad "room 7 area after the move is $(room 36 7 | val area), expected 644"
near "$(room 36 7 | val perimeter)" 102 1e-3 || bad "room 7 perimeter after the move is $(room 36 7 | val perimeter), expected 102"
near "$(room 36 8 | val area)" 364 1e-3 || bad "room 8 area after the move is $(room 36 8 | val area), expected 364"
near "$(room 36 8 | val perimeter)" 82 1e-3 || bad "room 8 perimeter after the move is $(room 36 8 | val perimeter), expected 82"
flat 36 | grep -qF '"name":"Kitchen"' || bad "room 7 lost its name"
# 4. floor from the room: 644 x 3 below the level
[ "$(nwalls 40)" -eq 6 ] || bad "expected 6 solids after Make floor, got $(nwalls 40)"
near "$(wall 40 9 | val volume)" 1932 1e-3 || bad "floor 9 volume is $(wall 40 9 | val volume), expected 1932"
near "$(wall 40 9 | val zmin)" -3 1e-6 || bad "floor 9 zmin is $(wall 40 9 | val zmin), expected -3"
near "$(wall 40 9 | val zmax)" 0 1e-6 || bad "floor 9 zmax is $(wall 40 9 | val zmax), expected 0"
# 5. the rectangle becomes floor 11; undo brings it back
near "$(wall 47 11 | val volume)" 1200 1e-3 || bad "floor 11 volume is $(wall 47 11 | val volume), expected 1200"
! flat 47 | grep -qF '"Rect":{' || bad "the rectangle is still there after Make floor"
flat 49 | grep -qF '"Rect":{' || bad "the rectangle did not come back after undo"
[ -z "$(wall 49 11)" ] || bad "floor 11 is still there after undo"
[ "$(nwalls 49)" -eq 6 ] || bad "expected 6 solids after undo, got $(nwalls 49)"

[ "$fail" -eq 0 ] && echo "MCP PHASE4C PASS" || { echo "outputs in $OUT"; exit 1; }
