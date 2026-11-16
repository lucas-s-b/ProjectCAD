#!/usr/bin/env bash
# End-to-end test of the shell feature through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_shell.out
rm -f "$OUT" target/mcp_shell_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"shell","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 40 x 20 x 10
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":40,"y":20,"world":true}'
  cw 6 Model
  call 7 click '{"x":20,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  call 10 state '{}'
  # 2. Shell face, then a click on the top face; in the 1280 x 800 window with the fitted camera pixel (600, 356) is the middle of the top face
  cw 11 "Shell face"
  call 12 key '{"key":"F"}'
  call 13 state '{}'
  call 14 click '{"x":600,"y":356}'
  call 15 state '{}'
  call 16 screenshot '{}'
  # 3. the thickness field of the selected shell is the second "Thickness" field
  st 17 Thickness "1 mm" 1
  call 18 state '{}'
  # 4. undo the thickness, then the shell
  call 19 key '{"key":"Z","ctrl":true}'
  call 20 state '{}'
  call 21 key '{"key":"Z","ctrl":true}'
  call 22 state '{}'
} | target/debug/cad-app.exe --mcp > "$OUT" 2> target/mcp_shell.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
# $1 and $2 equal within relative tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t * (b < 0 ? -b : b))}'; }

[ "$(grep -c . "$OUT")" -ge 21 ] || bad "expected 21 responses, got $(grep -c . "$OUT")"
grep '"id":16,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_shell_3d.png 2>/dev/null
[ "$(head -c 4 target/mcp_shell_3d.png | tail -c 3)" = "PNG" ] || bad "3D screenshot is not a PNG"

near "$(volume 10)" 8000 1e-9 || bad "box volume is $(volume 10)"
flat 13 | grep -q '"pick_face":true' || bad "Shell face did not start picking"
# the click opens the top face: 8000 - 36 * 16 * 8
near "$(volume 15)" 3392 1e-6 || bad "shell volume is $(volume 15), expected 3392"
states 15 | grep -q '{"1":"Ok","2":"Ok"}' || bad "states after the shell are $(states 15)"
flat 15 | grep -q '"pick_face":false' || bad "picking did not end"
# thickness 1 mm: 8000 - 38 * 18 * 9
near "$(volume 18)" 1844 1e-6 || bad "volume with thickness 1 mm is $(volume 18), expected 1844"
near "$(volume 20)" 3392 1e-6 || bad "volume after the first undo is $(volume 20), expected 3392"
near "$(volume 22)" 8000 1e-9 || bad "volume after the second undo is $(volume 22), expected 8000"
states 22 | grep -q '{"1":"Ok"}' || bad "states after the undo are $(states 22)"

[ "$fail" -eq 0 ] && echo "MCP SHELL PASS" || { echo "outputs in $OUT"; exit 1; }
