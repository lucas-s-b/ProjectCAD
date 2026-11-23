#!/usr/bin/env bash
# End-to-end test of linear patterns and mirrors through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_pattern.out
rm -f "$OUT" target/mcp_pattern_*.png target/mcp_pattern.stl

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"pattern","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. plate 100 x 60 x 5 with a hole of radius 3 at (10, 10)
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":100,"y":60,"world":true}'
  cw 6 Circle
  call 7 click '{"x":10,"y":10,"world":true}'
  call 8 click '{"x":13,"y":10,"world":true}'
  call 9 key '{"key":"Escape"}'
  cw 10 Model
  call 11 click '{"x":50,"y":0,"world":true}'
  st 12 Distance "5 mm"
  cw 13 Extrude
  call 14 click '{"x":13,"y":10,"world":true}'
  cw 15 Operation
  cw 16 Cut
  cw 17 Extrude
  call 18 state '{}'
  # 2. linear pattern of the cut: 4 holes, 20 mm apart
  cw 19 "Extrude 2"
  cw 20 "Linear pattern"
  st 21 Count 4
  st 22 dx "20 mm"
  call 23 state '{}'
  # 3. a pattern of that pattern: 3 rows, 15 mm apart
  cw 24 "Linear pattern 3"
  cw 25 "Linear pattern"
  st 26 Count 3
  st 27 dx 0
  st 28 dy "15 mm"
  call 29 state '{}'
  # 4. count from a parameter
  cw 30 "Param name"
  call 31 type_text '{"text":"n"}'
  cw 32 "Param expression"
  call 33 type_text '{"text":"2"}'
  cw 34 Add
  cw 35 "Linear pattern 3"
  st 36 Count n
  call 37 state '{}'
  st 38 "Param n" 4
  call 39 state '{}'
  # 5. mirror all bodies across YZ, then undo
  cw 40 "Mirror bodies"
  call 41 state '{}'
  call 42 key '{"key":"Z","ctrl":true}'
  call 43 state '{}'
  # 6. look at it, export it
  cw 44 3D
  call 45 key '{"key":"F"}'
  call 46 screenshot '{}'
  call 47 export_project '{"path":"target/mcp_pattern.stl"}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_pattern.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
volume() { flat "$1" | grep -oE '"volume":[-0-9.e+]+' | head -1 | cut -d: -f2; }
triangles() { flat "$1" | grep -oE '"triangles":[0-9]+' | head -1 | cut -d: -f2; }
states() { flat "$1" | grep -oE '"feature_states":\{[^}]*\}'; }
# $1 and $2 equal within relative tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t * (b < 0 ? -b : b))}'; }
# $1 holes: the plate volume minus that many holes of the first cut
holes() { awk -v v="$v1" -v n="$1" 'BEGIN {printf "%.9f", 30000 - n * (30000 - v)}'; }

[ "$(grep -c . "$OUT")" -ge 44 ] || bad "expected 44 responses, got $(grep -c . "$OUT")"
grep '"id":46,' "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_pattern_3d.png 2>/dev/null
[ "$(head -c 4 target/mcp_pattern_3d.png | tail -c 3)" = "PNG" ] || bad "3D screenshot is not a PNG"

# 1. plate with one hole
v1=$(volume 18)
states 18 | grep -q '{"1":"Ok","2":"Ok"}' || bad "states after the cut are $(states 18)"
near "$v1" 30000 0.01 && ! near "$v1" 30000 1e-4 || bad "volume with one hole is $v1"
# 2. four holes
states 23 | grep -q '{"1":"Ok","2":"Ok","3":"Ok"}' || bad "states after the pattern are $(states 23)"
near "$(volume 23)" "$(holes 4)" 1e-6 || bad "volume with 4 holes is $(volume 23), expected $(holes 4)"
# 3. twelve holes
states 29 | grep -q '{"1":"Ok","2":"Ok","3":"Ok","4":"Ok"}' || bad "states after the second pattern are $(states 29)"
near "$(volume 29)" "$(holes 12)" 1e-6 || bad "volume with 12 holes is $(volume 29), expected $(holes 12)"
# 4. n = 2 gives 2 x 3 holes, n = 4 gives 4 x 3
flat 37 | grep -qE '"param_values":\{"n":2(\.0)?\}' || bad "param n is not 2"
near "$(volume 37)" "$(holes 6)" 1e-6 || bad "volume with n = 2 is $(volume 37), expected $(holes 6)"
near "$(volume 39)" "$(holes 12)" 1e-6 || bad "volume with n = 4 is $(volume 39), expected $(holes 12)"
states 39 | grep -q '{"1":"Ok","2":"Ok","3":"Ok","4":"Ok"}' || bad "states with n = 4 are $(states 39)"
# 5. mirror bodies doubles the volume; undo restores it
states 41 | grep -q '"5":"Ok"' || bad "mirror is not Ok ($(states 41))"
near "$(volume 41)" "$(awk -v v="$(volume 39)" 'BEGIN {printf "%.9f", 2 * v}')" 1e-6 || bad "volume after the mirror is $(volume 41), expected twice $(volume 39)"
near "$(volume 43)" "$(volume 39)" 1e-9 || bad "volume after the undo is $(volume 43), expected $(volume 39)"
# 6. STL: 84 byte header plus 50 bytes per triangle
n=$(triangles 43)
[ -n "$n" ] && [ "$(wc -c < target/mcp_pattern.stl)" -eq $((84 + 50 * n)) ] || bad "STL size is $(wc -c < target/mcp_pattern.stl), expected $((84 + 50 * n))"

[ "$fail" -eq 0 ] && echo "MCP PATTERN PASS" || { echo "outputs in $OUT"; exit 1; }
