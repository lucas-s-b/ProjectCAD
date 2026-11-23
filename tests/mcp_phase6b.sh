#!/usr/bin/env bash
# End-to-end test of joint limits, motion studies and collision during motion, through `cad-app --mcp` only. No OS input, no screen capture.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase6b.out
CSV=target/mcp_phase6b.csv
rm -f "$OUT" "$CSV" target/mcp_phase6b_*.png

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }

{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"phase6b","version":"0"}}}'
  printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  # 1. box 10 x 20 x 5 (feature 1 "Extrude 1")
  cw 3 Rect
  call 4 click '{"x":0,"y":0,"world":true}'
  call 5 click '{"x":10,"y":20,"world":true}'
  cw 6 Model
  call 7 click '{"x":5,"y":0,"world":true}'
  st 8 Distance "5 mm"
  cw 9 Extrude
  # 2. components #1 (grounded) and #2
  cw 10 Assembly
  cw 11 "Component body"
  cw 12 "Body 1: Extrude 1"
  cw 13 "Add component"
  cw 14 "Add component"
  cw 15 "Extrude 1 #1"
  cw 16 Grounded
  # 3. revolute joint 1: #2 on #1 about Z through (10, 0, 0), limits -30 .. 90 deg
  cw 17 "Extrude 1 #2"
  cw 18 "Joint parent"
  cw 19 "Parent: Extrude 1 #1"
  cw 20 "Add joint"
  cw 21 "Joint type"
  cw 22 "Type: Revolute"
  st 23 "Parent origin X" "10 mm"
  st 24 "Joint value" "0 deg"
  st 25 "Joint min" "-30 deg"
  st 26 "Joint max" "90 deg"
  call 27 state '{}'
  # 4. a value above the maximum fails the joint walk; back to 0
  st 28 "Joint value" "120 deg"
  call 29 state '{}'
  st 30 "Joint value" "0 deg"
  # 5. study 0 .. 90 deg in 3 steps tracing corner (10, 0, 0) of #2, collisions checked
  st 31 "Motion end" "90 deg"
  st 32 "Motion steps" 3
  st 33 "Trace X" "10 mm"
  cw 34 "Check collisions"
  cw 35 "Run study"
  call 36 state '{}'
  call 37 widgets '{}'
  cw 38 3D
  call 39 key '{"key":"F"}'
  call 40 screenshot '{}'
  # 6. playback: step 3, play, pause
  st 41 "Motion step" 3
  call 42 state '{}'
  cw 43 Play
  call 44 state '{}'
  cw 45 Pause
  call 46 state '{}'
  # 7. CSV
  call 47 export_motion "{\"path\":\"$CSV\"}"
  # 8. an end above the maximum is refused; a new run; any edit drops it
  st 48 "Motion end" "120 deg"
  cw 49 "Run study"
  call 50 state '{}'
  st 51 "Motion end" "90 deg"
  cw 52 "Run study"
  call 53 state '{}'
  st 54 "Joint value" "10 deg"
  call 55 state '{}'
} | target/debug/cad-app.exe --mcp --background > "$OUT" 2> target/mcp_phase6b.err

fail=0
bad() { echo "FAIL: $1"; fail=1; }
# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# number field $1 of the row on stdin; element $2 of the flat array field $1
num() { grep -oE "\"$1\":[-0-9.e]+" | head -1 | cut -d: -f2; }
elem() { grep -oE "\"$1\":\[[-0-9.e,]*\]" | head -1 | sed "s/\"$1\":\[//; s/\]//" | cut -d, -f"$2"; }
# the motion object and everything after it (its own keys come first, alphabetically); the collision object
mot() { flat "$1" | grep -oE '"motion":\{.*'; }
mcol() { flat "$1" | grep -oE '"collision":\{[^}]*\}'; }

[ "$(grep -c . "$OUT")" -ge 54 ] || bad "expected at least 54 responses, got $(grep -c . "$OUT")"
grep "\"id\":40," "$OUT" | grep -o '"data":"[^"]*"' | sed 's/"data":"//; s/"$//' | base64 -d > target/mcp_phase6b_40.png 2>/dev/null
[ "$(head -c 4 target/mcp_phase6b_40.png | tail -c 3)" = "PNG" ] || bad "screenshot 40 is not a PNG"

# 3. the joint with its limits
flat 27 | grep -qF '"error":null,"id":1,"kind":"Revolute","max":"90deg","min":"-30deg"' || bad "joint 1 of state 27"
# 4. above the maximum
flat 29 | grep -qF '"error":"joint1:120degisabovethemaximum90deg"' || bad "joint error of state 29"
# 5. the study
mot 36 | grep -qF '"motion":{"accuracy":"exact","check_ms":' || bad "motion of state 36"
mot 36 | grep -qF '"checked":true' || bad "study 36 not checked"
mcol 36 | grep -qF '"moving":2,"other":1,"sample":1,' || bad "collision of state 36: $(mcol 36)"
near "$(mcol 36 | num volume)" 535.898385 0.0001 || bad "collision volume is $(mcol 36 | num volume), expected 535.898385"
mot 36 | grep -qF '"collision_accuracy":"approximate(sampledatthesteps)"' || bad "collision accuracy"
[ "$(mot 36 | num step)" = 1 ] || bad "shown step is $(mot 36 | num step), expected 1"
[ "$(mot 36 | num steps)" = 3 ] || bad "steps is $(mot 36 | num steps), expected 3"
mot 36 | grep -qF '"unit":"deg"' || bad "unit"
near "$(mot 36 | num value)" 30 0.000001 || bad "value at step 1 is $(mot 36 | num value)"
near "$(mot 36 | elem values 4)" 90 0.000001 || bad "last value"
near "$(mot 36 | elem point 1)" 18.660254 0.000001 || bad "point x"
near "$(mot 36 | elem point 2)" 5 0.000001 || bad "point y"
near "$(mot 36 | elem point 3)" 0 0.000001 || bad "point z"
grep "\"id\":37," "$OUT" | grep -qF 'approximate (sampled at the steps)' || bad "no sampled label in widgets 37"
grep "\"id\":37," "$OUT" | grep -qF 'Run study' || bad "no 'Run study' in widgets 37"
# 6. playback
[ "$(mot 42 | num step)" = 3 ] || bad "step of state 42 is $(mot 42 | num step), expected 3"
near "$(mot 42 | num value)" 90 0.000001 || bad "value at step 3"
near "$(mot 42 | elem point 1)" 10 0.000001 || bad "point x at step 3"
near "$(mot 42 | elem point 2)" 10 0.000001 || bad "point y at step 3"
mot 44 | grep -qF '"playing":true' || bad "not playing in state 44"
mot 46 | grep -qF '"playing":false' || bad "still playing in state 46"
# 7. CSV
[ "$(wc -l < "$CSV")" -eq 5 ] || bad "CSV has $(wc -l < "$CSV") lines, expected 5"
head -1 "$CSV" | grep -qF 'Step,Value (deg),X (mm),Y (mm),Z (mm),Collision' || bad "CSV header"
[ "$(sed -n 2p "$CSV")" = "0,0.000000,20.000000,0.000000,0.000000," ] || bad "CSV row 0: $(sed -n 2p "$CSV")"
sed -n 3p "$CSV" | grep -qF '1,30.000000,18.660254,5.000000,0.000000,Extrude 1 #2 / Extrude 1 #1 interference 535.898 mm' || bad "CSV row 1: $(sed -n 3p "$CSV")"
# 8. refused run, new run, edit
flat 50 | grep -qF '"motion":null' || bad "motion not null in state 50"
flat 50 | grep -qF 'end:120degisabovethemaximum90deg' || bad "no limit error in state 50"
flat 53 | grep -qF '"motion":{"accuracy":"exact"' || bad "no motion in state 53"
flat 55 | grep -qF '"motion":null' || bad "motion not dropped by the edit in state 55"

[ "$fail" -eq 0 ] && echo "MCP PHASE6B PASS" || { echo "outputs in $OUT"; exit 1; }
