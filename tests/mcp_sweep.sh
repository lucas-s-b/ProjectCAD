#!/usr/bin/env bash
# End-to-end test of sweep and loft through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_sweep.out
rm -f "$OUT" target/mcp_sweep_*.png target/mcp_loft.stl

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"text\":\"$3\"}"; }
wc_() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true${4:+,\"shift\":true}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"sweep","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. L-shaped path in sketch 0
  cw 3 Line
  wc_ 4 0 0
  wc_ 5 30 0
  call 6 key '{"key":"Escape"}'
  cw 7 Line
  wc_ 8 30 0
  wc_ 9 30 20
  call 10 key '{"key":"Escape"}'
  call 11 state '{}'
  # 2. profile circle in a YZ sketch
  cw 12 Model
  st 13 "Sketch offset" 0
  cw 14 "Base plane"
  cw 15 YZ
  cw 16 "Add sketch"
  cw 17 Circle
  wc_ 18 0 0
  wc_ 19 2 0
  call 20 key '{"key":"Escape"}'
  wc_ 21 2 0
  cw 22 "Capture selection"
  call 23 state '{}'
  # 3. the path
  cw 24 "Sketch 0"
  wc_ 25 15 0
  wc_ 26 30 10 shift
  cw 27 "Capture selection"
  call 28 state '{}'
  # 4. sweep, then look at it
  cw 29 Sweep
  call 30 state '{}'
  cw 31 3D
  call 32 key '{"key":"F"}'
  call 33 screenshot '{}'
  # 5. loft: two squares
  call 34 new_project '{}'
  cw 35 2D
  cw 36 Rect
  wc_ 37 -10 -10
  wc_ 38 10 10
  call 39 key '{"key":"Escape"}'
  wc_ 40 0 -10
  cw 41 Model
  cw 42 "Capture selection"
  st 43 "Sketch offset" "15 mm"
  cw 44 "Base plane"
  cw 45 XY
  cw 46 "Add sketch"
  cw 47 Rect
  wc_ 48 -5 -5
  wc_ 49 5 5
  call 50 key '{"key":"Escape"}'
  wc_ 51 0 -5
  cw 52 "Capture selection"
  cw 53 Loft
  call 54 state '{}'
  cw 55 3D
  call 56 key '{"key":"F"}'
  call 57 screenshot '{}'
  call 58 export_project '{"path":"target/mcp_loft.stl"}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_sweep.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
triangles() { flat "$1" | grep -oE '"triangles":[0-9]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
captures() { flat "$1" | grep -oE '"captures":\[[^]]*\]'; }
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t * (b < 0 ? -b : b))}'; }

[ "$(grep -c . "$OUT")" -ge 57 ] || bad "expected 57 responses, got $(grep -c . "$OUT")"
for p in "33 sweep" "57 loft"; do
  set -- $p
  grep "\"id\":$1," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > "target/mcp_sweep_$2.png" 2>/dev/null
  [ "$(head -c 4 "target/mcp_sweep_$2.png" | tail -c 3)" = "PNG" ] || bad "$2 screenshot is not a PNG"
done

[ "$(captures 23)" = '"captures":[1]' ] || bad "captures after the profile are $(captures 23)"
[ "$(captures 28)" = '"captures":[1,2]' ] || bad "captures after the path are $(captures 28)"
states 30 | grep -q '{"1":"Ok"}' || bad "sweep states are $(states 30)"
v=$(volume 30)
# 2 %, not 1 %: the circle is a polygon, which alone is 1.1 % below pi r^2 (the kernel test uses 2 %)
exact=$(awk 'BEGIN {printf "%.9f", 3.14159265358979 * 4 * 50}')
near "$v" "$exact" 0.02 && awk -v v="$v" -v e="$exact" 'BEGIN {exit !(v < e)}' || bad "sweep volume is $v, expected just below $exact"
[ "$(captures 30)" = '"captures":[]' ] || bad "captures after the sweep are $(captures 30)"
states 54 | grep -q '{"1":"Ok"}' || bad "loft states are $(states 54)"
near "$(volume 54)" 3500 1e-6 || bad "loft volume is $(volume 54), expected 3500"
n=$(triangles 54)
[ -n "$n" ] && [ "$(wc -c < target/mcp_loft.stl)" -eq $((84 + 50 * n)) ] || bad "STL size is $(wc -c < target/mcp_loft.stl), expected $((84 + 50 * n))"

[ "$fail" -eq 0 ] && echo "MCP SWEEP PASS" || { echo "outputs in $OUT"; exit 1; }
