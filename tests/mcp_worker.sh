#!/usr/bin/env bash
# End-to-end test of the rebuild worker through `cad-app --mcp` only: a plate with a 10 x 10 hole grid, then an edit
# whose reply, screenshot and state must all wait for the rebuild. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_worker.out
rm -f "$OUT" target/mcp_worker_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"worker","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. plate 55 x 55 x 5 with a hole of radius 1 at (5, 5)
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":55,"y":55,"world":true}'
  cw 6 Circle
  call 7 click '{"x":5,"y":5,"world":true}'
  call 8 click '{"x":6,"y":5,"world":true}'
  call 9 key '{"key":"Escape"}'
  cw 10 Model
  call 11 click '{"x":27,"y":0,"world":true}'
  st 12 Distance "5 mm"
  cw 13 Extrude
  call 14 click '{"x":6,"y":5,"world":true}'
  cw 15 Operation
  cw 16 Cut
  cw 17 Extrude
  call 18 state '{}'
  # 2. two linear patterns of 10, 5 mm apart: 100 holes
  cw 19 "Extrude 2"
  cw 20 "Linear pattern"
  st 21 Count 10
  st 22 dx "5 mm"
  cw 23 "Linear pattern 3"
  cw 24 "Linear pattern"
  st 25 Count 10
  st 26 dx 0
  st 27 dy "5 mm"
  call 28 state '{}'
  # 3. widen the holes; the reply, the screenshot and the state all come after the rebuild
  cw 29 Sketch
  cw 30 Select
  call 31 click '{"x":6,"y":5,"world":true}'
  st 32 radius 1.5
  call 33 screenshot '{}'
  call 34 state '{}'
  # 4. back to the first volume
  call 35 key '{"key":"Z","ctrl":true}'
  call 36 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_worker.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
# $1 and $2 equal within relative tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t * (b < 0 ? -b : b))}'; }

[ "$(grep -c . "$OUT")" -ge 35 ] || bad "expected 35 responses, got $(grep -c . "$OUT")"
grep '"id":33,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_worker_shot.png 2>/dev/null
[ "$(head -c 4 target/mcp_worker_shot.png | tail -c 3)" = "PNG" ] || bad "screenshot is not a PNG"

# 1. one hole, then 100
v1=$(volume 18)
near "$v1" 15125 0.01 && ! near "$v1" 15125 1e-4 || bad "volume with one hole is $v1"
v100=$(volume 28)
expect=$(awk -v v="$v1" 'BEGIN {printf "%.9f", 15125 - 100 * (15125 - v)}')
near "$v100" "$expect" 1e-6 || bad "volume with 100 holes is $v100, expected $expect"
states 28 | grep -q '{"1":"Ok","2":"Ok","3":"Ok","4":"Ok"}' || bad "states are $(states 28)"
# 2. radius 1.5 removes 2.25 times the material of radius 1 (the tessellations differ a little)
v2=$(volume 34)
ratio=$(awk -v p=15125 -v a="$v100" -v b="$v2" 'BEGIN {printf "%.6f", (p - b) / (p - a)}')
near "$ratio" 2.25 0.02 || bad "removed volume ratio is $ratio, expected 2.25 (volumes $v100 and $v2)"
flat 34 | grep -q '"rebuilding":false' || bad "state still says rebuilding"
flat 36 | grep -q '"rebuilding":false' || bad "state after the undo says rebuilding"
# 3. undo restores the first volume
near "$(volume 36)" "$v100" 1e-9 || bad "volume after the undo is $(volume 36), expected $v100"

[ "$fail" -eq 0 ] && echo "MCP WORKER PASS" || { echo "outputs in $OUT"; exit 1; }
