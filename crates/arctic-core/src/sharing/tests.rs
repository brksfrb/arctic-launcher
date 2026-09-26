use std::io::Write;

use serde_json::json;

use super::*;
use crate::instances::Loader;

fn instance_bundle(extra: Value) -> Value {
    let mut data = json!({
        "name": "Friends SMP",
        "game_version": "1.21.4",
        "loader": {"type": "fabric", "version": "0.16.10"},
        "mods": [
            {"project": "AANobbMI", "version": "u6dRKJwZ", "title": "Sodium"},
            {"project": "P7dR8mSH", "version": "zgJTWkNn", "title": "Fabric API", "enabled": false, "dependency": true}
        ]
    });
    for (k, v) in extra.as_object().unwrap() {
        data[k] = v.clone();
    }
    json!({"arctic_share": 1, "kind": "instance", "data": data})
}

#[test]
fn instance_round_trips_through_text_and_json() {
    let bundle = Bundle::from_value(&instance_bundle(json!({}))).unwrap();
    let Bundle::Instance(pack) = &bundle else {
        panic!("not an instance")
    };
    assert_eq!(pack.mods.len(), 2);
    assert!(pack.mods[0].enabled && !pack.mods[1].enabled);
    let text = bundle.to_text();
    assert!(text.starts_with("arctic1."));
    assert_eq!(read(&text).unwrap(), Input::Bundle(bundle.clone()));
    // Line breaks from chat apps don't matter.
    let wrapped = format!("{}\n{}", &text[..20], &text[20..]);
    assert_eq!(read(&wrapped).unwrap(), Input::Bundle(bundle.clone()));
    assert_eq!(
        read(&bundle.to_file_text()).unwrap(),
        Input::Bundle(bundle.clone())
    );
    assert!(bundle.summary().contains("2 mods"));
}

#[test]
fn java_settings_never_come_in() {
    let v = instance_bundle(json!({
        "jvm_args": "-javaagent:evil.jar",
        "java_path": "C:\\evil\\java.exe",
        "max_memory_mb": 999999
    }));
    let Bundle::Instance(pack) = Bundle::from_value(&v).unwrap() else {
        panic!()
    };
    let out = serde_json::to_string(&pack).unwrap();
    assert!(!out.contains("evil"));
    assert_eq!(pack.max_memory_mb, None);
}

#[test]
fn bad_instances_are_refused() {
    for extra in [
        json!({"name": ""}),
        json!({"name": "x".repeat(49)}),
        json!({"game_version": "../../etc"}),
        json!({"loader": {"type": "fabric", "version": "1;rm -rf"}}),
        json!({"mods": [{"project": "../x", "version": "abc", "title": "x"}]}),
        json!({"loader": {"type": "vanilla"}}),
    ] {
        assert!(
            Bundle::from_value(&instance_bundle(extra.clone())).is_err(),
            "{extra}"
        );
    }
}

#[test]
fn hud_is_rebuilt_with_safe_values() {
    let v = json!({"arctic_share": 1, "kind": "hud", "data": {
        "hud": {"fps": {"enabled": true, "placed": true, "ax": 2, "dx": 10, "scale": 1.5, "junk": 1}},
        "hudVersion": 2,
        "proxy": {"host": "steal.me"},
        "hiddenPlayers": ["x"]
    }});
    let Bundle::Client(part) = Bundle::from_value(&v).unwrap() else {
        panic!()
    };
    assert_eq!(part.part, Part::Hud);
    assert!(part.values.get("proxy").is_none());
    assert!(part.values.get("hiddenPlayers").is_none());
    let fps = &part.values["hud"]["fps"];
    assert_eq!(fps["ay"], 0);
    assert_eq!(fps["background"], true);
    assert!(fps.get("junk").is_none());
    assert_eq!(part.widgets_on(), Some(1));
    for bad in [
        json!({"hud": {"fps": {"scale": 50.0}}}),
        json!({"hud": {"fps": {"ax": 7}}}),
        json!({"hud": {"Bad Id!": {}}}),
        json!({"hud": "nope"}),
        json!({}),
    ] {
        let v = json!({"arctic_share": 1, "kind": "hud", "data": bad});
        assert!(Bundle::from_value(&v).is_err(), "{v}");
    }
}

#[test]
fn crosshair_and_keys_are_checked() {
    let ok = json!({"arctic_share": 1, "kind": "client", "data": {
        "crosshair": {"enabled": true, "style": "dot", "size": 3, "color": -1},
        "zoomKey": "key.keyboard.z",
        "style": "aurora"
    }});
    let Bundle::Client(part) = Bundle::from_value(&ok).unwrap() else {
        panic!()
    };
    assert_eq!(part.values["crosshair"]["gap"], 2);
    for bad in [
        json!({"crosshair": {"size": 99}}),
        json!({"crosshair": {"style": "skull"}}),
        json!({"zoomKey": "rm -rf"}),
        json!({"style": "evil"}),
    ] {
        let v = json!({"arctic_share": 1, "kind": "client", "data": bad});
        assert!(Bundle::from_value(&v).is_err(), "{v}");
    }
}

#[test]
fn applying_keeps_the_rest_of_the_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = client::config_path(dir.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"proxy": {"host": "mine"}, "fullbright": true, "hud": {}}"#,
    )
    .unwrap();
    let part = ClientPart::check(
        Part::Hud,
        &json!({"hud": {"cps": {"enabled": true}}, "hudVersion": 2}),
    )
    .unwrap();
    part.apply(dir.path()).unwrap();
    let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["proxy"]["host"], "mine");
    assert_eq!(saved["fullbright"], true);
    assert_eq!(saved["hud"]["cps"]["enabled"], true);
    let back = ClientPart::read(dir.path(), Part::All).unwrap();
    assert!(back.values.get("proxy").is_none());
}

#[test]
fn profile_settings_are_filtered_and_checked() {
    let v = json!({"arctic_share": 1, "kind": "profile", "data": {
        "settings": {"theme": "light", "extra_jvm_args": "-javaagent:x", "java_override": "C:\\x", "max_memory_mb": 6144},
        "instances": []
    }});
    let Bundle::Profile(p) = Bundle::from_value(&v).unwrap() else {
        panic!()
    };
    assert_eq!(p.settings.len(), 2);
    let bad =
        json!({"arctic_share": 1, "kind": "profile", "data": {"settings": {"theme": "purple"}}});
    assert!(Bundle::from_value(&bad).is_err());
    let no_version = json!({"arctic_share": 1, "kind": "profile", "data": {"settings": {},
        "instances": [{"name": "A", "game_version": null, "loader": {"type": "vanilla"}}]}});
    assert!(Bundle::from_value(&no_version).is_err());
}

#[test]
fn pasted_input_is_recognized() {
    assert_eq!(read(" ABCD-EFGH ").unwrap(), Input::Code("abcdefgh".into()));
    assert!(read("hello there").is_err());
    assert!(read("arctic1.@@@").is_err());
    assert!(read("{\"arctic_share\": 9, \"kind\": \"hud\"}").is_err());
    assert!(read("{\"arctic_share\": 1, \"kind\": \"worm\"}").is_err());
    assert_eq!(codes::pretty("abcdefgh"), "abcd-efgh");
    let _ = Loader::Vanilla;
}

#[test]
fn inflating_stops_at_the_limit() {
    // Megabytes of zeros deflate to a few KB; reading must stop early.
    let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(&vec![b' '; 8 * 1024 * 1024]).unwrap();
    let packed = enc.finish().unwrap();
    let text = format!("arctic1.{}", URL_SAFE_NO_PAD.encode(packed));
    let err = read(&text).unwrap_err().to_string();
    assert!(err.contains("too big"), "{err}");
}

#[test]
fn settings_of_a_running_game_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    let lock = std::fs::File::create(dir.path().join("config/arctic.lock")).unwrap();
    let part =
        ClientPart::check(Part::Crosshair, &json!({"crosshair": {"enabled": true}})).unwrap();
    assert!(!client::in_use(dir.path()));
    // Another handle holding the lock stands in for the running game.
    lock.lock().unwrap();
    assert!(client::in_use(dir.path()));
    assert!(part.apply(dir.path()).is_err());
    lock.unlock().unwrap();
    part.apply(dir.path()).unwrap();
}
