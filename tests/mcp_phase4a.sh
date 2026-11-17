#!/usr/bin/env bash
# End-to-end test of levels and joined walls, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase4a.out
rm -f "$OUT" target/mcp_phase4a_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
wc_() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true${4:+,\"shift\":true}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase4a","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. level 1 (elevation 0, height 25 mm); walls 2 thick: A (0,0)-(40,0) and B (40,0)-(40,30) make an L,
  #    C (20,0)-(20,30) a T on A (the first click snaps to A's midpoint)
  cw 3 Model
  st 4 "Level height" "25 mm"
  cw 5 "Add level"
  cw 6 Wall
  st 7 "Wall thickness" 2
  wc_ 8 0 0
  wc_ 9 40 0
  wc_ 10 40 30
  call 11 key '{"key":"Escape"}'
  cw 12 Wall
  wc_ 13 20 0
  wc_ 14 20 30
  call 15 key '{"key":"Escape"}'
  call 16 state '{}'
  # 2. C made 4 thick in the Properties panel (Sketch tab)
  cw 17 Sketch
  wc_ 18 20 15
  st 19 thickness 4
  call 20 state '{}'
  call 21 screenshot '{}'
  # 3. level 2 at 25 mm with one free wall
  cw 22 Model
  st 23 "Level elevation" "25 mm"
  cw 24 "Add level"
  cw 25 Wall
  wc_ 26 0 0
  wc_ 27 40 0
  call 28 key '{"key":"Escape"}'
  call 29 state '{}'
  cw 30 3D
  call 31 key '{"key":"F"}'
  call 32 screenshot '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_phase4a.err

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

[ "$(grep -c . "$OUT")" -ge 31 ] || bad "expected at least 31 responses, got $(grep -c . "$OUT")"
for n in 21 32; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase4a_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase4a_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. L and T: outlines 80 + 60 + 58 mm2 (L mitre: (41 + 39) / 2 x 2 and (31 + 29) / 2 x 2; the T wall 2 x 29) times 25
[ "$(nwalls 16)" -eq 3 ] || bad "expected 3 walls after step 1, got $(nwalls 16)"
near "$(wvol 16)" 4950 1e-3 || bad "wall volume after step 1 is $(wvol 16), expected 4950"
flat 16 | grep -q '"level":{"height":"25mm"}' || bad "level height is not stored"
# 2. C becomes 4 x 29 = 116: (80 + 60 + 116) x 25
near "$(wvol 20)" 6400 1e-3 || bad "wall volume after the thickness edit is $(wvol 20), expected 6400"
flat 20 | grep -q '"thickness":4.0' || bad "thickness 4 is not in the state"
# 3. second level: + 40 x 2 x 25
[ "$(nwalls 29)" -eq 4 ] || bad "expected 4 walls after step 3, got $(nwalls 29)"
near "$(wvol 29)" 8400 1e-3 || bad "wall volume after step 3 is $(wvol 29), expected 8400"
flat 29 | grep -q '"zmax":50.0,"zmin":25.0' || bad "level 2 wall is not between z 25 and 50"

[ "$fail" -eq 0 ] && echo "MCP PHASE4A PASS" || { echo "outputs in $OUT"; exit 1; }
