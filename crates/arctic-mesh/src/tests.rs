use super::*;
use serde_json::json;

/// A GLB with `json` and `bin`.
fn glb(json: &Value, bin: &[u8]) -> Vec<u8> {
    let mut j = serde_json::to_vec(json).unwrap();
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    let mut b = bin.to_vec();
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    let total = 12 + 8 + j.len() + if b.is_empty() { 0 } else { 8 + b.len() };
    let mut out = Vec::new();
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&j);
    if !b.is_empty() {
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(&b);
    }
    out
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// One triangle on a node called `body`, 0 to 10 pixels, optionally animated.
fn triangle_doc(animate: bool) -> (Value, Vec<u8>) {
    let mut bin = floats(&[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0]);
    let mut views = vec![json!({"buffer":0,"byteOffset":0,"byteLength":36})];
    let mut accessors = vec![json!({"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"})];
    let mut doc = json!({
        "asset":{"version":"2.0"},
        "scene":0,"scenes":[{"nodes":[0]}],
        "nodes":[{"name":"body","mesh":0}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
        "buffers":[{"byteLength":0}],
    });
    if animate {
        let at = bin.len();
        bin.extend(floats(&[0.0, 1.0, 2.0]));
        bin.extend(floats(&[
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            0.0,
            1.0,
        ]));
        views.push(json!({"buffer":0,"byteOffset":at,"byteLength":12}));
        views.push(json!({"buffer":0,"byteOffset":at + 12,"byteLength":48}));
        accessors.push(json!({"bufferView":1,"componentType":5126,"count":3,"type":"SCALAR"}));
        accessors.push(json!({"bufferView":2,"componentType":5126,"count":3,"type":"VEC4"}));
        doc["animations"] = json!([{"samplers":[{"input":1,"output":2}],"channels":[{"sampler":0,"target":{"node":0,"path":"rotation"}}]}]);
    }
    doc["bufferViews"] = Value::Array(views);
    doc["accessors"] = Value::Array(accessors);
    (doc, bin)
}

fn triangle(animate: bool) -> Vec<u8> {
    let (doc, bin) = triangle_doc(animate);
    glb(&doc, &bin)
}

#[test]
fn the_sample_file_parses() {
    assert_eq!(parse(&sample_glb()).unwrap().triangles(), 1);
}

/// The triangle file with its JSON edited.
fn rewrite(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let (mut doc, bin) = triangle_doc(false);
    edit(&mut doc);
    glb(&doc, &bin)
}

#[test]
fn reads_a_triangle_with_flat_normals() {
    let m = parse(&triangle(false)).unwrap();
    assert_eq!(m.nodes.len(), 1);
    assert_eq!(m.nodes[0].name, "body");
    assert_eq!(m.triangles(), 1);
    let p = &m.primitives[0];
    assert_eq!(p.positions.len(), 3);
    // Counter-clockwise in x then y: the normal points to +z.
    assert!(p.normals.iter().all(|n| (n[2] - 1.0).abs() < 1e-6));
    assert_eq!(p.colors[0], [1.0; 4]);
    assert_eq!(m.bounds(), ([0.0, 0.0, 0.0], [10.0, 10.0, 0.0]));
    assert!(m.animation.is_none());
}

#[test]
fn animation_loops_and_has_a_rest_pose() {
    let m = parse(&triangle(true)).unwrap();
    let anim = m.animation.as_ref().unwrap();
    assert_eq!(anim.length, 2.0);
    // Rest pose: the node's own transform (no animation).
    assert_eq!(apply(&m.world(None)[0], [1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
    // A quarter turn about z at t = 1: x goes to y.
    let p = apply(&m.world(Some(1.0))[0], [1.0, 0.0, 0.0]);
    assert!(p[0].abs() < 1e-3 && (p[1] - 1.0).abs() < 1e-3, "{p:?}");
    // Looping: t = 2 + 1 is t = 1.
    let again = apply(&m.world(Some(3.0))[0], [1.0, 0.0, 0.0]);
    assert!((again[1] - p[1]).abs() < 1e-3);
}

#[test]
fn refuses_what_is_not_supported() {
    assert!(parse(b"nope").is_err());
    let mut bad = triangle(false);
    bad[8] ^= 1; // the declared length
    assert!(parse(&bad).unwrap_err().contains("length"));
    for (key, value) in [
        ("extensionsUsed", json!(["KHR_draco_mesh_compression"])),
        ("skins", json!([{"joints":[0]}])),
        ("cameras", json!([{}])),
    ] {
        let doc = rewrite(|d| d[key] = value.clone());
        assert!(parse(&doc).is_err(), "{key}");
    }
    let uri = rewrite(|d| d["buffers"][0]["uri"] = json!("http://example.com/x.bin"));
    assert!(parse(&uri).unwrap_err().contains("URI"));
    let matrix = rewrite(|d| {
        d["nodes"][0]["matrix"] = json!([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
    });
    assert!(parse(&matrix).unwrap_err().contains("matrix"));
    let attr = rewrite(|d| d["meshes"][0]["primitives"][0]["attributes"]["JOINTS_0"] = json!(0));
    assert!(parse(&attr).unwrap_err().contains("JOINTS_0"));
    let lines = rewrite(|d| d["meshes"][0]["primitives"][0]["mode"] = json!(1));
    assert!(parse(&lines).unwrap_err().contains("triangle"));
    let far = rewrite(|d| d["nodes"][0]["translation"] = json!([9999.0, 0.0, 0.0]));
    assert!(parse(&far).is_err());
    let looped = rewrite(|d| {
        d["nodes"] = json!([{"children":[1]}, {"children":[0], "mesh":0}]);
    });
    assert!(parse(&looped).is_err());
}

#[test]
fn refuses_nonsense_geometry() {
    // An index past the vertices.
    let mut bin = floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    bin.extend([0u8, 1, 9, 0]);
    let doc = json!({
        "asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
        "nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],
        "buffers":[{"byteLength":40}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":3}],
        "accessors":[
            {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
            {"bufferView":1,"componentType":5121,"count":3,"type":"SCALAR"}]
    });
    assert!(
        parse(&glb(&doc, &bin))
            .unwrap_err()
            .contains("past the vertices")
    );
    // An accessor that runs past its data.
    let short = rewrite(|d| d["accessors"][0]["count"] = json!(30));
    assert!(parse(&short).unwrap_err().contains("past"));
    // A huge count is refused without reading anything.
    let huge = rewrite(|d| d["accessors"][0]["count"] = json!(100_000_000));
    assert!(parse(&huge).unwrap_err().contains("elements"));
    // NaN coordinates.
    let mut nan = triangle(false);
    let at = nan.len() - 4;
    nan[at..].copy_from_slice(&f32::NAN.to_le_bytes());
    let _ = parse(&nan); // must not panic
}

#[test]
fn too_large_is_refused_before_reading() {
    let mut big = triangle(false);
    big.resize(MAX_BYTES + 1, 0);
    assert!(parse(&big).unwrap_err().contains("larger"));
}
