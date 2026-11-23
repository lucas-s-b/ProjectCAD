#!/usr/bin/env bash
# End-to-end test of a tapered extrude and a smooth loft, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase3e.out
rm -f "$OUT" target/mcp_phase3e_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
wc_() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true${4:+,\"shift\":true}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase3e","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. square 20 x 20, extruded 5 mm with a 45 deg taper
  cw 3 Rect
  wc_ 4 -10 -10
  wc_ 5 10 10
  call 6 key '{"key":"Escape"}'
  wc_ 7 0 -10
  cw 8 Model
  st 9 Distance "5 mm"
  st 10 Taper "45 deg"
  cw 11 Extrude
  call 12 state '{}'
  cw 13 3D
  call 14 key '{"key":"F"}'
  call 15 screenshot '{}'
  # 2. smooth loft through squares of half width 5, 10, 5 at z 0, 10, 20
  call 16 new_project '{}'
  cw 17 2D
  cw 18 Rect
  wc_ 19 -5 -5
  wc_ 20 5 5
  call 21 key '{"key":"Escape"}'
  wc_ 22 0 -5
  cw 23 Model
  cw 24 "Capture selection"
  st 25 "Sketch offset" "10 mm"
  cw 26 "Base plane"
  cw 27 XY
  cw 28 "Add sketch"
  cw 29 Rect
  wc_ 30 -10 -10
  wc_ 31 10 10
  call 32 key '{"key":"Escape"}'
  wc_ 33 0 -10
  cw 34 "Capture selection"
  st 35 "Sketch offset" "20 mm"
  cw 36 "Base plane"
  cw 37 XY
  cw 38 "Add sketch"
  cw 39 Rect
  wc_ 40 -5 -5
  wc_ 41 5 5
  call 42 key '{"key":"Escape"}'
  wc_ 43 0 -5
  cw 44 "Capture selection"
  cw 45 Smooth
  cw 46 Loft
  call 47 state '{}'
  cw 48 3D
  call 49 key '{"key":"F"}'
  call 50 screenshot '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase3e.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }

[ "$(grep -c . "$OUT")" -ge 49 ] || bad "expected at least 49 responses, got $(grep -c . "$OUT")"
for n in 15 50; do
  grep "\"id\":$n," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase3e_$n.png 2>/dev/null
  [ "$(head -c 4 target/mcp_phase3e_$n.png | tail -c 3)" = "PNG" ] || bad "screenshot $n is not a PNG"
done

# 1. frustum of 20 and 10 over 5 mm: 5/3 (400 + 100 + 200)
states 12 | grep -q '{"1":"Ok"}' || bad "states after the taper are $(states 12)"
near "$(volume 12)" 1166.667 1e-3 || bad "tapered volume is $(volume 12), expected 1166.667"
# 2. smooth loft; the ruled one would give 4666.67 (then the Smooth click did not reach the feature)
states 47 | grep -q '{"1":"Ok"}' || bad "states after the loft are $(states 47)"
near "$(volume 47)" 5207.98 0.01 || bad "smooth loft volume is $(volume 47), expected 5207.98"

[ "$fail" -eq 0 ] && echo "MCP PHASE3E PASS" || { echo "outputs in $OUT"; exit 1; }
