#!/usr/bin/env bash
# End-to-end test of Phase 8 batch B through `cad-app --mcp --background` only (no OS input): construction lines and
# rays, intersection snaps far from their anchor, the parallel snap, the ray switch in Properties, DXF export and the
# 3D scene size.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase8b.out
DXF=target/mcp_phase8b.dxf
rm -f "$OUT" "$DXF"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
at() { call "$1" click "{\"x\":$2,\"y\":$3,\"world\":true}"; }
key() { call "$1" key "{\"key\":\"$2\"${3:+,$3}}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase8b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. two construction lines through the origin (45 and -45 degrees), then a ray along y = 20
  cw 2 XLine
  at 3 0 0
  at 4 10 10
  at 5 10 -10
  key 6 Escape
  cw 7 Ray
  at 8 0 20
  at 9 10 20
  key 10 Escape
  call 11 state '{}'
  # 2. a line from the intersection of the 45 degree line and the ray at (20,20), clicked a little off it
  cw 20 Line
  at 21 20.3 19.8
  at 22 40 -30
  call 23 state '{}'
  # 3. parallel snap: start at (-30,10), drag along the ray to acquire it, then click near the parallel
  at 30 -30 10
  call 31 drag '{"x1":30,"y1":20.2,"x2":34,"y2":20.4,"world":true}'
  at 32 5 10.4
  key 33 Escape
  call 34 state '{}'
  # 4. Properties: the 45 degree line becomes a ray
  cw 40 Select
  at 41 30 30
  cw 42 "ray (one side only)"
  call 43 state '{}'
  # 5. DXF export leaves construction lines out; the 3D scene stays small
  call 50 export_project "{\"path\":\"$DXF\"}"
  cw 51 3D
  call 52 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase8b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# entity $2 of state $1 from its id on (construction flag, shape)
ent() { flat "$1" | grep -oE "\"$2\":\{\"construction\":[a-z]+,\"group\":[a-z0-9]+,\"id\":$2,\"layer\":[0-9]+,\"meta\":\{\},\"name\":\"[^\"]*\",\"shape\":\{\"[A-Za-z]+\":\{[^}]*\}"; }
nums() { ent "$1" "$2" | sed 's/.*"shape"://' | grep -oE -- '-?[0-9][0-9.e-]*' | tr '\n' ' '; }
is() {
  local st=$1 id=$2; shift 2
  local got; got=($(nums "$st" "$id"))
  [ "${#got[@]}" -eq "$#" ] || return 1
  local i=0
  for want in "$@"; do near "${got[$i]}" "$want" 1e-6 || return 1; i=$((i + 1)); done
}
ents() { flat "$1" | grep -oE '"entities":[0-9]+' | head -1 | cut -d: -f2; }
r=0.7071067811865476

[ "$(ents 11)" = 3 ] || bad "construction lines: $(ents 11) entities, want 3"
ent 11 1 | grep -qF '"construction":true' && ent 11 1 | grep -qF '"XLine":{' && ent 11 1 | grep -qF '"ray":false' || bad "first XLine: $(ent 11 1)"
is 11 1 $r $r 0 0 || bad "first XLine numbers: $(nums 11 1)"
is 11 2 $r -$r 0 0 || bad "second XLine numbers: $(nums 11 2)"
ent 11 3 | grep -qF '"ray":true' && is 11 3 1 0 0 20 || bad "ray: $(ent 11 3)"
is 23 4 20 20 40 -30 || bad "line from the intersection snap: $(nums 23 4)"
is 34 5 -30 10 5 10 || bad "line from the parallel snap: $(nums 34 5)"
ent 43 1 | grep -qF '"ray":true' || bad "Properties ray switch: $(ent 43 1)"
[ -s "$DXF" ] || bad "no DXF written"
grep -qE 'XLINE|RAY|10000000' "$DXF" && bad "construction lines in the DXF"
v=$(flat 52 | grep -oE '"scene_vertices":[0-9]+' | cut -d: -f2)
[ -n "$v" ] && [ "$v" -lt 20000 ] || bad "3D scene vertices: $v"
grep -q '"isError":true' "$OUT" && bad "a tool call failed: $(grep '"isError":true' "$OUT" | head -1 | cut -c1-200)"

[ "$fail" -eq 0 ] && echo "MCP PHASE8B PASS" || { echo "see $OUT"; exit 1; }
