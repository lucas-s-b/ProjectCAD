#!/usr/bin/env bash
# End-to-end test of closed-loop joints (a four-bar linkage), its motion study and dragging a joint value, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase6c.out
rm -f "$OUT" target/mcp_phase6c_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase6c","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 10 x 20 x 5 (feature 1 "Extrude 1"), as in 6b
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":10,"y":20,"world":true}'
  cw 6 Model
  call 7 click '{"x":5,"y":0,"world":true}'
  st 8 Distance "5 mm"
  cw 9 Extrude
  # 2. components #1 (grounded) to #4
  cw 10 Assembly
  cw 11 "Component body"
  cw 12 "Body 1: Extrude 1"
  cw 13 "Add component"
  cw 14 "Add component"
  cw 15 "Add component"
  cw 16 "Add component"
  cw 17 "Extrude 1 #1"
  cw 18 Grounded
  # 3. joint 1, crank: #2 on #1, revolute about Z at the origin, 0 deg (the input)
  cw 19 "Extrude 1 #2"
  cw 20 "Joint parent"
  cw 21 "Parent: Extrude 1 #1"
  cw 22 "Add joint"
  cw 23 "Joint type"
  cw 24 "Type: Revolute"
  st 25 "Joint value" "0 deg"
  # 4. joint 2, coupler: #3 on #2 at (20, 0, 0), free, start 40 deg
  cw 26 "Extrude 1 #3"
  cw 27 "Joint parent"
  cw 28 "Parent: Extrude 1 #2"
  cw 29 "Add joint"
  cw 30 "Joint type"
  cw 31 "Type: Revolute"
  st 32 "Parent origin X" "20 mm"
  st 33 "Joint value" "40 deg"
  cw 34 "Free joint"
  # 5. joint 3, rocker: #4 on #1 at (50, 0, 0), free, start 60 deg; no loop yet: under-constrained
  cw 35 "Extrude 1 #4"
  cw 36 "Joint parent"
  cw 37 "Parent: Extrude 1 #1"
  cw 38 "Add joint"
  cw 39 "Joint type"
  cw 40 "Type: Revolute"
  st 41 "Parent origin X" "50 mm"
  st 42 "Joint value" "60 deg"
  cw 43 "Free joint"
  call 44 state '{}'
  # 6. joint 4 closes the loop (#4 already has a parent): (60, 0, 0) on the coupler meets (40, 0, 0) on the rocker
  cw 45 "Joint parent"
  cw 46 "Parent: Extrude 1 #3"
  cw 47 "Add joint"
  cw 48 "Joint type"
  cw 49 "Type: Revolute"
  st 50 "Parent origin X" "60 mm"
  st 51 "Child origin X" "40 mm"
  call 52 state '{}'
  call 53 widgets '{}'
  # 7. study: crank 0 .. 360 deg in 36 steps (the default count), tracing (40, 0, 0) of the rocker
  st 54 "Motion end" "360 deg"
  cw 55 "Trace component"
  cw 56 "Trace: Extrude 1 #4"
  st 57 "Trace X" "40 mm"
  cw 58 "Run study"
  st 59 "Motion step" 9
  call 60 state '{}'
  st 61 "Motion step" 18
  call 62 state '{}'
  cw 63 3D
  call 64 key '{"key":"F"}'
  call 65 screenshot '{}'
  # 8. drag the crank to 30 deg by typing in the joint editor
  cw 66 "Joint 1"
  st 67 "Joint drag value" 30
  call 68 state '{}'
  # 9. the crank free too: under-constrained
  cw 69 "Free joint"
  call 70 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase6c.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin; element $2 of the flat array field $1
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
elem() { grep -oE "\"$1\":\[[-0-9.e,]*\]" | head -1 | sed "s/\"$1\":\[//; s/\]//" | cut -d, -f"$2"; }
# the motion object and everything after it (its own keys come first, alphabetically)
mot() { flat "$1" | grep -oE '"motion":\{.*'; }
# the joint row with id $2 in state $1
jrow() { flat "$1" | grep -oE '\{"child":[0-9]+,[^{}]*\}' | grep -E "\"id\":$2,"; }

[ "$(grep -c . "$OUT")" -ge 69 ] || bad "expected at least 69 responses, got $(grep -c . "$OUT")"
grep "\"id\":65," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase6c_65.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase6c_65.png | tail -c 3)" = "PNG" ] || bad "screenshot 65 is not a PNG"

# 5. free joints without a loop
jrow 44 2 | grep -qF '"error":"joint2:under-constrained(degreesoffreedomleft:2);markfewerjointsfree","free":true,"id":2' || bad "joint 2 of state 44: $(jrow 44 2)"
jrow 44 3 | grep -qF '"free":true,"id":3' || bad "joint 3 of state 44: $(jrow 44 3)"
# 6. the loop closes
jrow 52 4 | grep -qF '{"child":4,"closes_loop":true,"error":null,"id":4,"kind":"Revolute"' || bad "joint 4 of state 52: $(jrow 52 4)"
near "$(jrow 52 2 | num solved)" 36.336058 0.000001 || bad "solved joint 2 is $(jrow 52 2 | num solved), expected 36.336058"
near "$(jrow 52 3 | num solved)" 62.720387 0.000001 || bad "solved joint 3 is $(jrow 52 3 | num solved), expected 62.720387"
jrow 52 1 | grep -qF '{"child":2,"error":null,"id":1,' || bad "joint 1 of state 52 is not a plain row: $(jrow 52 1)"
grep "\"id\":53," "$OUT" | grep -qF 'Closes loop' || bad "no 'Closes loop' in widgets 53"
grep "\"id\":53," "$OUT" | grep -qF 'approximate (loop closed to 1e-9 mm)' || bad "no solved label in widgets 53"
# 7. the study
mot 60 | grep -qF '"motion":{"accuracy":"approximate(loopclosedto1e-9mm)"' || bad "motion of state 60: $(mot 60 | head -c 120)"
[ "$(mot 60 | num step)" = 9 ] || bad "step of state 60 is $(mot 60 | num step), expected 9"
[ "$(mot 60 | num steps)" = 36 ] || bad "steps of state 60 is $(mot 60 | num steps), expected 36"
near "$(mot 60 | num value)" 90 0.000001 || bad "value at step 9 is $(mot 60 | num value)"
near "$(mot 60 | elem values 37)" 360 0.000001 || bad "last value is $(mot 60 | elem values 37)"
near "$(mot 60 | elem point 1)" 56.769224 0.00001 || bad "point x at step 9 is $(mot 60 | elem point 1)"
near "$(mot 60 | elem point 2)" 39.423059 0.00001 || bad "point y at step 9 is $(mot 60 | elem point 2)"
near "$(mot 60 | elem point 3)" 0 0.000001 || bad "point z at step 9 is $(mot 60 | elem point 3)"
[ "$(mot 62 | num step)" = 18 ] || bad "step of state 62 is $(mot 62 | num step), expected 18"
near "$(mot 62 | elem point 1)" 29.285714 0.00001 || bad "point x at step 18 is $(mot 62 | elem point 1)"
near "$(mot 62 | elem point 2)" 34.218684 0.00001 || bad "point y at step 18 is $(mot 62 | elem point 2)"
# 8. dragging by typing
jrow 68 1 | grep -qF '"value":"30deg"' || bad "joint 1 value of state 68: $(jrow 68 1)"
near "$(jrow 68 2 | num solved)" -7.590985 0.00001 || bad "solved joint 2 at 30 deg is $(jrow 68 2 | num solved)"
near "$(jrow 68 3 | num solved)" 55.267777 0.00001 || bad "solved joint 3 at 30 deg is $(jrow 68 3 | num solved)"
flat 68 | grep -qF '"motion":null' || bad "motion not dropped by the edit in state 68"
# 9. the crank free too
jrow 70 1 | grep -qF '"error":"joint4:under-constrained(degreesoffreedomleft:1);markfewerjointsfree","free":true,"id":1' || bad "joint 1 of state 70: $(jrow 70 1)"
! flat 70 | grep -qF '"solved":' || bad "solved values left in state 70"

[ "$fail" -eq 0 ] && echo "MCP PHASE6C PASS" || { echo "outputs in $OUT"; exit 1; }
