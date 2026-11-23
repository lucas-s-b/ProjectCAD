#!/usr/bin/env bash
# End-to-end test of the plugin API: apply_commands, the Plugins menu and the cad-plugin-api client with its example plugin, through `cad-app --mcp` only. No OS input.
cd "$(dirname "$0")/.." || exit 1
OUT=target/mcp_phase7e.out
DIR=target/mcp_phase7e
SET=$DIR/settings.json
SAVED=target/mcp_phase7e_flange.cadproj
rm -rf "$DIR"
rm -f "$OUT" "$SAVED" target/mcp_phase7e.err target/mcp_phase7e_plugin.err
mkdir -p "$DIR/plugins"

call() { printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3"; }
# click the widget with this name; the optional third argument is its occurrence index (draw order)
cw() { call "$1" click_widget "{\"name\":\"$2\",\"index\":${3:-0}}"; }
st() { call "$1" set_text "{\"name\":\"$2\",\"index\":${4:-0},\"text\":\"$3\"}"; }
# init <client name>: the initialize request and notification of one app launch
init() { printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"$1\",\"version\":\"0\"}}}" '{"jsonrpc":"2.0","method":"notifications/initialized"}'; }

fail=0
bad() { echo "FAIL: $1"; fail=1; }

EX=target/debug/examples/bolt_circle.exe
[ -x "$EX" ] || { echo "build the example first: cargo build -p cad-plugin-api --examples"; exit 1; }
# the manifest needs a Windows path (Git Bash: pwd -W gives D:/...)
printf '{"name":"Bolt circle","command":"%s/%s","args":["--count","8"],"version":"1"}\n' "$(pwd -W)" "$EX" > "$DIR/plugins/bolt.json"
printf '{"name":"Broken","command":"cad-no-such-program-7e.exe","version":"1"}\n' > "$DIR/plugins/broken.json"
printf '{' > "$DIR/plugins/bad.json"
# 1. the client starts the app itself (target/debug/cad-app.exe --mcp --background) and saves the flange
"$EX" --app target/debug/cad-app.exe --save "$SAVED" 2> target/mcp_phase7e_plugin.err || bad "standalone plugin: $(cat target/mcp_phase7e_plugin.err)"

C1='{"Add":{"id":1,"name":"c1","shape":{"Circle":{"center":[0,0],"radius":5}}}}'
C2='{"Add":{"id":2,"name":"c2","shape":{"Circle":{"center":[20,0],"radius":7}}}}'
C3='{"Add":{"id":3,"name":"c3","shape":{"Circle":{"center":[0,20],"radius":4}}}}'
{ init phase7e
  # 2. apply_commands: query, two circles as one undo step, strict refusals with rollback
  call 3 apply_commands '{"commands":[]}'
  call 4 apply_commands "{\"commands\":[$C1,$C2]}"
  call 5 state '{}'
  call 6 key '{"key":"Z","ctrl":true}'
  call 7 state '{}'
  call 8 apply_commands '{"commands":[{"Explode":1}]}'
  call 9 apply_commands '{"commands":[{"Add":{"id":5}}]}'
  call 10 apply_commands '{"commands":[],"undo":true}'
  call 11 apply_commands "{\"commands\":[$C3,$C3]}"
  call 12 state '{}'
  call 13 apply_commands '{"commands":[{"Remove":99}]}'
  # 3. the Plugins menu: the bolt circle plugin (8 holes) runs inside this app; state waits until it exits
  call 14 state '{}'
  cw 15 Plugins
  cw 16 "Plugin: Bolt circle"
  call 17 state '{}'
  call 18 key '{"key":"Z","ctrl":true}'
  call 19 state '{}'
  cw 20 Plugins
  cw 21 "Plugin: Broken"
  call 22 state '{}'
  # 4. the flange of the standalone run
  call 23 open_project "{\"path\":\"$SAVED\"}"
  call 24 state '{}'
} | CAD_SETTINGS="$SET" target/debug/cad-app.exe --mcp --background >> "$OUT" 2>> target/mcp_phase7e.err
{ init phase7e
  # 5. without CAD_SETTINGS an MCP run has no plugins folder
  call 101 state '{}'
  cw 102 Plugins
  call 103 widgets '{}'
} | target/debug/cad-app.exe --mcp --background >> "$OUT" 2>> target/mcp_phase7e.err

# state is pretty-printed JSON inside a JSON string: strip escapes and whitespace before matching
flat() { grep "\"id\":$1," "$OUT" | sed 's/[\]n//g; s/[\]"/"/g; s/ //g'; }
# $1 and $2 equal within absolute tolerance $3
near() { awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(a != "" && d <= t)}'; }
# $1 >= $2
ge() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a != "" && a >= b)}'; }
# the bodies array of state $1
bv() { flat "$1" | grep -oE '"bodies":\[[^]]*\]'; }
# the volume of body $2 in state $1
vol() { bv "$1" | grep -oE "\"id\":$2,\"volume\":[-0-9.e]+" | head -1 | cut -d: -f3; }
# the entity count of state $1
ents() { flat "$1" | grep -oE '"entities":[0-9]+' | head -1 | cut -d: -f2; }
# the reply line of request $1
rl() { grep "\"id\":$1," "$OUT"; }
# the plugins object of state $1
pl() { flat "$1" | grep -oE '"plugins":\{"available":\[[^]]*\],"errors":\[[^]]*\],"running":[^}]*\}'; }

[ "$(grep -c . "$OUT")" -ge 27 ] || bad "expected at least 27 responses, got $(grep -c . "$OUT")"

# 1: the standalone run
grep -q '"version": 28' "$SAVED" || bad "saved file version"
[ "$(grep -c '"Circle"' "$SAVED")" -eq 7 ] || bad "circles in the saved file: $(grep -c '"Circle"' "$SAVED")"
[ "$(grep -c '"Extrude"' "$SAVED")" -eq 7 ] || bad "extrudes in the saved file: $(grep -c '"Extrude"' "$SAVED")"
grep -q "6 holes" target/mcp_phase7e_plugin.err || bad "standalone plugin message: $(cat target/mcp_phase7e_plugin.err)"
# 2: apply_commands
flat 3 | grep -qF '"applied":0,"features":0,"format_version":28,"next":{"component":1,"constraint":1,"entity":1,"feature":1,"group":1,"joint":1,"layer":1,"sketch":1},"undo":[]' || bad "query reply: $(flat 3)"
flat 4 | grep -qF '"applied":2' && flat 4 | grep -qF '"entity":3' && flat 4 | grep -qF '"undo":[{"Remove":2},{"Remove":1}]' || bad "apply reply: $(flat 4)"
[ "$(ents 5)" = 2 ] || bad "entities after apply: $(ents 5)"
[ "$(ents 7)" = 0 ] || bad "entities after one undo: $(ents 7)"
rl 8 | grep -q '"isError":true' && rl 8 | grep -q 'command 0: unknown variant' || bad "unknown variant: $(rl 8)"
rl 9 | grep -q 'command 0: missing field' || bad "missing field: $(rl 9)"
rl 10 | grep -q "unknown argument 'undo'" || bad "unknown argument: $(rl 10)"
rl 11 | grep -q 'entity 3 already exists' || bad "duplicate id: $(rl 11)"
[ "$(ents 12)" = 0 ] || bad "entities after the rolled-back call: $(ents 12)"
rl 13 | grep -q 'entity 99 not found' || bad "missing entity: $(rl 13)"
# 3: the Plugins menu
pl 14 | grep -qF '"available":["Boltcircle","Broken"]' && pl 14 | grep -qF '"errors":["plugins/bad.json:' && pl 14 | grep -qF '"running":null' || bad "plugins in state 14: $(pl 14)"
flat 17 | grep -qF '"status":"PluginBoltcirclefinished"' || bad "status after the plugin: $(flat 17 | grep -oE '"status":"[^"]*"')"
[ "$(ents 17)" = 9 ] || bad "entities after the plugin: $(ents 17)"
near "$(vol 17 1)" 109076.1 1091 || bad "plugin flange volume: $(bv 17)"
pl 17 | grep -qF '"running":null' || bad "plugin still running: $(pl 17)"
[ "$(ents 19)" = 0 ] || bad "entities after undoing the plugin: $(ents 19)"
grep -q "bolt circle: 8 holes" target/mcp_phase7e.err || bad "plugin message: $(cat target/mcp_phase7e.err)"
flat 22 | grep -qF '"status":"PluginBrokennotstarted:' || bad "status of the broken plugin: $(flat 22 | grep -oE '"status":"[^"]*"')"
# 4: the standalone flange
[ "$(ents 24)" = 7 ] || bad "entities of the saved flange: $(ents 24)"
near "$(vol 24 1)" 110081.4 1101 || bad "saved flange volume: $(bv 24)"
# 5: no plugins folder without CAD_SETTINGS
[ "$(pl 101)" = '"plugins":{"available":[],"errors":[],"running":null}' ] || bad "plugins without CAD_SETTINGS: $(pl 101)"
rl 103 | grep -q 'Plugin:' && bad "plugin entry without a plugins folder"

[ "$fail" -eq 0 ] && echo "MCP PHASE7E PASS" || { echo "see $OUT"; exit 1; }
