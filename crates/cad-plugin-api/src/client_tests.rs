use super::*;

#[test]
fn request_lines_are_json_rpc() {
    let line = request_line(7, "state", &json!({}));
    assert!(!line.contains('\n'));
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v, json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"state","arguments":{}}}));
}

#[test]
fn replies_match_their_id() {
    let l = r#"{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"ok"}],"isError":false}}"#;
    let r = parse_message(l, 3).unwrap().unwrap();
    let t = tool_reply(&r).unwrap();
    assert_eq!((t.text.as_str(), t.image), ("ok", None));
    assert!(parse_message(l, 4).unwrap().is_none());
    assert!(parse_message(r#"{"jsonrpc":"2.0","method":"notifications/x"}"#, 3).unwrap().is_none());
    let rpc = r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"method not found"}}"#;
    assert!(matches!(parse_message(rpc, 3), Err(PluginError::Rpc { code: -32601, .. })));
    assert!(matches!(parse_message("not json", 3), Err(PluginError::Protocol(_))));
    let e = r#"{"content":[{"type":"text","text":"entity 9 not found"}],"isError":true}"#;
    let Err(PluginError::Tool(t)) = tool_reply(&serde_json::from_str(e).unwrap()) else { panic!("not a tool error") };
    assert_eq!(t, "entity 9 not found");
    let img = r#"{"content":[{"type":"text","text":"2x1"},{"type":"image","data":"Zm9v","mimeType":"image/png"}],"isError":false}"#;
    assert_eq!(tool_reply(&serde_json::from_str(img).unwrap()).unwrap().image, Some(b"foo".to_vec()));
}

#[test]
fn base64_decodes_the_apps_encoding() {
    for (i, o) in [("", ""), ("Zg==", "f"), ("Zm8=", "fo"), ("Zm9v", "foo"), ("Zm9vYmFy", "foobar")] {
        assert_eq!(base64_decode(i).unwrap(), o.as_bytes());
    }
    assert!(base64_decode("Zm9v!").is_err());
}

#[test]
fn applied_reply_parses() {
    let s = r#"{"applied":2,"features":0,"format_version":28,"next":{"component":1,"constraint":1,"entity":3,"feature":1,"group":1,"joint":1,"layer":1,"sketch":1},"undo":[{"Remove":2},{"Remove":1}]}"#;
    let a: Applied = serde_json::from_str(s).unwrap();
    assert_eq!((a.applied, a.next.entity, a.undo.len()), (2, 3, 2));
    assert!(matches!(a.undo[0], Command::Remove(2)));
}

#[test]
fn pointer_arguments() {
    let m = pointer_args(&Pointer { button: Button::Secondary, world: true, shift: false, ctrl: true });
    assert_eq!(Value::Object(m), json!({"button":"secondary","world":true,"shift":false,"ctrl":true}));
    let m = Value::Object(pointer_args(&Pointer::default()));
    assert_eq!(m, json!({"button":"primary","world":false,"shift":false,"ctrl":false}));
}
