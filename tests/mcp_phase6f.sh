#!/usr/bin/env bash
# End-to-end test of the validation report and the safety factor, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase6f.out
rm -f "$OUT" target/mcp_phase6f.cadproj

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

# The faces of the box in the "Study face" list, as in mcp_phase6e.sh.
FMX="Face 5: plane -X"
FPX="Face 3: plane +X"

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase6f","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 20 x 10 x 10 (body 1), as in mcp_phase6e.sh; no benchmark run yet
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":20,"y":10,"world":true}'
  cw 6 Model
  call 7 click '{"x":10,"y":0,"world":true}'
  st 8 Distance "10 mm"
  cw 9 Extrude
  call 10 state '{}'
  # 2. the benchmarks
  cw 11 "Run benchmarks"
  call 12 state '{}'
  call 13 widgets '{}'
  # 3. steel study made Custom with Poisson's ratio 0, h 5 mm; fix -X, 1000 N along +X on +X; run
  cw 14 "New static study"
  st 15 "Poisson's ratio" "0"
  st 16 "Study element size" "5 mm"
  cw 17 "Study face"
  cw 18 "$FMX"
  cw 19 "Fix face"
  cw 20 "Study face"
  cw 21 "$FPX"
  st 22 "Force X" "1000"
  st 23 "Force Z" "0"
  cw 24 "Add force"
  cw 25 "Run static study"
  call 26 state '{}'
  call 27 widgets '{}'
  # 4. no yield strength (a blank: set_text with an empty text types nothing and leaves the old text): no safety factor
  st 28 "Yield strength" " "
  cw 29 "Run static study"
  call 30 state '{}'
  # 5. 355 MPa; saved as format 27
  st 31 "Yield strength" "355"
  cw 32 "Run static study"
  call 33 state '{}'
  call 34 save_project '{"path":"target/mcp_phase6f.cadproj"}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase6f.err

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
# the fea_study object of state $1 (arrays, no nested braces)
fs() { flat "$1" | grep -oE '"fea_study":\{[^}]*\}'; }
# the top of the validation object of state $1 (failed, ms, passed sort before rows)
va() { flat "$1" | grep -oE '"validation":\{"failed":[0-9]+,"ms":[-0-9.e]+,"passed":[0-9]+'; }

[ "$(grep -c . "$OUT")" -ge 33 ] || bad "expected at least 33 responses, got $(grep -c . "$OUT")"

# 10. no benchmark run yet
flat 10 | grep -qF '"validation":null' || bad "validation of state 10 is not null"
# 12. the benchmarks
[ "$(va 12 | num failed)" = 0 ] || bad "failed of state 12 is $(va 12 | num failed): $(va 12)"
[ "$(va 12 | num passed)" = 15 ] || bad "passed of state 12 is $(va 12 | num passed)"
ge "$(va 12 | num ms)" 0 || bad "ms of state 12 is $(va 12 | num ms)"
[ "$(flat 12 | grep -o '"accuracy":"validatedagainst' | wc -l)" -eq 15 ] || bad "state 12 does not have 15 validated rows"
for t in '"case":"cantileverbeam"' '"case":"barintension"' '"case":"solidbox"' '"case":"solidcylinder"' '"case":"four-barpositionanalysis"'; do
  flat 12 | grep -qF "$t" || bad "no $t in state 12"
done
# 13. the panel
for t in 'Run benchmarks' '15 of 15 cases within tolerance' 'validated against cantilever beam' 'validated against four-bar position analysis'; do
  grep "\"id\":13," "$OUT" | grep -qF "$t" || bad "no '$t' in widgets 13"
done
# 26. the tension bar with steel's yield strength: sigma = 10 MPa, safety factor 25
fs 26 | grep -qF '"accuracy":"approximate(FEA,linearelastic,meshh=5mm)"' || bad "fea_study of state 26: $(fs 26)"
near "$(fs 26 | num max_von_mises)" 10 0.000001 || bad "max_von_mises of state 26 is $(fs 26 | num max_von_mises)"
near "$(fs 26 | num safety_factor)" 25 0.000001 || bad "safety_factor of state 26 is $(fs 26 | num safety_factor)"
near "$(fs 26 | num yield_strength)" 250 0 || bad "yield_strength of state 26 is $(fs 26 | num yield_strength)"
# 27. the panel
grep "\"id\":27," "$OUT" | grep -qF 'Yield strength' || bad "no 'Yield strength' in widgets 27"
# 30. no yield strength
fs 30 | grep -qF '"safety_factor":null' || bad "safety_factor of state 30: $(fs 30)"
fs 30 | grep -qF '"yield_strength":null' || bad "yield_strength of state 30: $(fs 30)"
# 33. 355 MPa
near "$(fs 33 | num safety_factor)" 35.5 0.000001 || bad "safety_factor of state 33 is $(fs 33 | num safety_factor)"
# the saved file
grep -qF '"version": 29' target/mcp_phase6f.cadproj || bad "the saved file is not version 29"
grep -qF '"yield_strength": "355"' target/mcp_phase6f.cadproj || bad "the saved file has no yield strength 355"

[ "$fail" -eq 0 ] && echo "MCP PHASE6F PASS" || { echo "outputs in $OUT"; exit 1; }
