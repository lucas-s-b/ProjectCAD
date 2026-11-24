#!/usr/bin/env bash
# End-to-end test of Phase 8 batch A through `cad-app --mcp --background` only (no OS input): the point-picked
# transform tools Move, Copy, Rotate..., Scale..., Mirror... and Stretch, with snapping and the coordinate field.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase8a.out
rm -f "$OUT"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
at() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true}"; }
key() { call "$1" key "{\"key\":\"$2\"${3:+,$3}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase8a","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # a line (0,0)-(40,0), left selected
  cw 2 Line
  at 3 0 0
  at 4 40 0
  key 5 Escape
  # 1. Move: base (0,0), target typed in the coordinate field as @10,5
  cw 10 Move
  at 11 0 0
  call 12 set_text '{"name":"Coordinates","text":"@10,5"}'
  key 13 Enter
  call 14 state '{}'
  # 2. Copy twice from one base point, then Escape
  cw 20 Copy
  at 21 10 5
  at 22 10 25
  at 23 10 45
  key 24 Escape
  call 25 state '{}'
  # 3. Rotate...: base (10,5), reference (50,5), target (10,65): a quarter turn
  cw 30 "Rotate…"
  at 31 10 5
  at 32 50 5
  at 33 10 65
  call 34 state '{}'
  # 4. Scale...: base (10,5), reference (10,45), target (10,85): factor 2
  cw 40 "Scale…"
  at 41 10 5
  at 42 10 45
  at 43 10 85
  call 44 state '{}'
  # 5. Mirror... keeping the original, across the line x = -50
  cw 50 "Mirror…"
  at 51 -50 0
  at 52 -50 10
  call 53 state '{}'
  key 54 Escape
  # 6. Stretch: a rectangle (60,-40)-(80,-30); the window takes its right side, which moves 10 to the right
  cw 60 Rect
  at 61 60 -40
  at 62 80 -30
  key 63 Escape
  cw 64 Stretch
  at 65 75 -45
  at 66 90 -25
  at 67 80 -40
  at 68 90 -40
  call 69 state '{}'
  # 7. Move with nothing selected says so
  key 70 Escape
  at 71 40 -70
  cw 72 Move
  at 73 0 0
  call 74 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase8a.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# shape numbers of entity $2 in state $1, space separated
shp() { flat "$1" | grep -oE "\"$2\":\{\"construction\":[a-z]+,\"group\":[a-z0-9]+,\"id\":$2,\"layer\":[0-9]+,\"meta\":\{\},\"name\":\"[^\"]*\",\"shape\":\{\"[A-Za-z]+\":\{[^}]*\}" | sed 's/.*"shape"://' | grep -oE -- '-?[0-9][0-9.e-]*' | tr '\n' ' '; }
kind() { flat "$1" | grep -oE "\"id\":$2,\"layer\":[0-9]+,\"meta\":\{\},\"name\":\"[^\"]*\",\"shape\":\{\"[A-Za-z]+\"" | grep -oE '"[A-Za-z]+"$'; }
# entity $2 of state $1 has the numbers $3... within 1e-6
is() {
  local st=$1 id=$2; shift 2
  local got; got=($(shp "$st" "$id"))
  [ "${#got[@]}" -eq "$#" ] || return 1
  local i=0
  for want in "$@"; do near "${got[$i]}" "$want" 1e-6 || return 1; i=$((i + 1)); done
}
ents() { flat "$1" | grep -oE '"entities":[0-9]+' | head -1 | cut -d: -f2; }

is 14 1 10 5 50 5 || bad "Move: $(shp 14 1)"
[ "$(ents 25)" = 3 ] || bad "Copy made $(ents 25) entities, want 3"
is 25 2 10 25 50 25 || bad "first copy: $(shp 25 2)"
is 25 3 10 45 50 45 || bad "second copy: $(shp 25 3)"
is 25 1 10 5 50 5 || bad "Copy moved the original: $(shp 25 1)"
is 34 1 10 5 10 45 || bad "Rotate: $(shp 34 1)"
is 44 1 10 5 10 85 || bad "Scale: $(shp 44 1)"
[ "$(ents 53)" = 4 ] || bad "Mirror with Keep original: $(ents 53) entities, want 4"
is 53 4 -110 5 -110 85 || bad "mirrored copy: $(shp 53 4)"
is 53 1 10 5 10 85 || bad "Mirror moved the original: $(shp 53 1)"
[ "$(kind 69 5)" = '"Rect"' ] || bad "stretched rectangle kind: $(kind 69 5)"
is 69 5 90 -30 60 -40 || bad "Stretch: $(shp 69 5)"
flat 74 | grep -qF '"status":"Selectwhattotransformfirst"' || bad "empty selection status: $(flat 74 | grep -oE '"status":"[^"]*"')"
grep -q '"isError":true' "$OUT" && bad "a tool call failed: $(grep '"isError":true' "$OUT" | head -1 | cut -c1-200)"

[ "$fail" -eq 0 ] && echo "MCP PHASE8A PASS" || { echo "see $OUT"; exit 1; }
