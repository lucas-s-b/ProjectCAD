#!/usr/bin/env bash
# End-to-end test of analyses and picking that used to hold the UI thread, through `cad-app --mcp` only. No OS input.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase7b.out
BEAM=target/mcp_phase7b_beam.cadproj
GEARS=target/mcp_phase7b_gears.cadproj
PIN=target/mcp_phase7b_pin.cadproj
ASM=target/mcp_phase7b_asm.cadproj
rm -f "$OUT" "$BEAM" "$GEARS" "$PIN" "$ASM"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

target/debug/cad-bench.exe beam "$BEAM" || { echo "cannot write $BEAM"; exit 1; }
target/debug/cad-bench.exe assembly "$GEARS" 20 80 || { echo "cannot write $GEARS"; exit 1; }
target/debug/cad-bench.exe pin "$PIN" || { echo "cannot write $PIN"; exit 1; }
target/debug/cad-bench.exe assembly "$ASM" 100 || { echo "cannot write $ASM"; exit 1; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase7b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. the beam and its study (about 29 766 unknowns)
  call 3 open_project "{\"path\":\"$BEAM\"}"
  cw 4 Model
  call 5 state '{}'
  cw 6 "Run static study"
  call 7 state '{}'
  # 2. the validation benchmarks
  cw 8 "Run benchmarks"
  call 9 state '{}'
  # 3. the analysis mesh of the beam at the study's element size
  st 10 "Element size" "2.45 mm"
  cw 11 "Mesh for analysis"
  call 12 state '{}'
  # 4. interference check of 20 gears 80 mm apart
  call 13 open_project "{\"path\":\"$GEARS\"}"
  cw 14 Assembly
  call 15 state '{}'
  cw 16 Check
  call 17 state '{}'
  # 5. one turn of the pin in 360 steps with the collision check
  call 18 open_project "{\"path\":\"$PIN\"}"
  cw 19 Assembly
  st 20 "Motion end" "360 deg"
  st 21 "Motion steps" "360"
  cw 22 "Check collisions"
  call 23 state '{}'
  cw 24 "Run study"
  call 25 state '{}'
  # 6. a click in the 3D view of 100 gears runs the component pick over every component
  call 26 open_project "{\"path\":\"$ASM\"}"
  cw 27 3D
  call 28 state '{}'
  call 29 click '{"x":575,"y":400}'
  call 30 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase7b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
# $1 >= $2
ge() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a >= b)}'; }
# the perf object of state $1
pf() { flat "$1" | grep -oE '"perf":\{[^}]*\}'; }
# the fea_study / fea_mesh object of state $1
fs() { flat "$1" | grep -oE '"fea_study":\{[^}]*\}'; }
fm() { flat "$1" | grep -oE '"fea_mesh":\{[^}]*\}'; }
# the motion object of state $1
mo() { flat "$1" | grep -oE '"motion":\{.*"values":\['; }

# the jobs object of state $1
jb() { flat "$1" | grep -oE '"jobs":\{[^}]*\}'; }
fin() { jb "$1" | num finished; }
lt() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a < b)}'; }
GATES="study motion check validation"
gated() { case " $GATES " in *" $1 "*) return 0;; esac; return 1; }

[ "$(grep -c . "$OUT")" -ge 29 ] || bad "expected at least 29 responses, got $(grep -c . "$OUT")"

# 7. the static study
ge "$(fs 7 | num dofs)" 25000 || bad "dofs of state 7: $(fs 7)"
fs 7 | grep -qF '"error":null' || bad "study error in state 7: $(fs 7)"
# 9. the validation benchmarks
flat 9 | grep -qF '"failed":0,"ms":' || bad "benchmarks failed in state 9"
flat 9 | grep -qF '"passed":15,"rows"' || bad "benchmarks passed in state 9"
# 12. the analysis mesh
ge "$(fm 12 | num elements)" 1 || bad "elements of state 12: $(fm 12)"
fm 12 | grep -qF '"error":null' || bad "mesh error in state 12: $(fm 12)"
# 17. the interference check
flat 17 | grep -qF '"meetings":[{"a":' || bad "no meetings in state 17"
# 25. the motion study and its collision check
mo 25 | grep -qF '"checked":true,"collision":null' || bad "collision check of state 25"
mo 25 | grep -qF '"steps":360,' || bad "steps of state 25"
# 30. the pick
ge "$(pf 30 | num frame_ms)" 0 || bad "frame_ms of state 30: $(pf 30)"

# Batch B: gated analyses ran on the worker (finished grew across the click) without a long frame; replies wait, so nothing is running.
for i in 5 7 9 12 15 17 23 25 28 30; do
  jb $i | grep -qF '"cancelled":0,' || bad "jobs cancelled in state $i: $(jb $i)"
  jb $i | grep -qF '"running":null' || bad "job running in state $i: $(jb $i)"
done
# kind: after-state before-state
chk() {
  if gated "$1"; then
    [ "$(fin $2)" -gt "$(fin $3)" ] && lt "$(pf $2 | num frame_ms)" 100 || bad "$1: $(jb $2) $(pf $2)"
  else
    [ "$(fin $2)" = "$(fin $3)" ] || bad "$1 ran as a job: $(jb $2) $(jb $3)"
  fi
}
chk study 7 5
chk validation 9 7
chk mesh 12 9
chk check 17 15
chk motion 25 23
if gated pick; then lt "$(pf 30 | num frame_ms)" 100 || bad "pick: $(pf 30)"; fi

for i in 7 9 12 17 25 30; do echo "frame_ms $i: $(pf $i | num frame_ms)"; done
[ "$fail" -eq 0 ] && echo "MCP PHASE7B PASS" || { echo "outputs in $OUT"; exit 1; }
