use super::*;

/// Encode an RGBA buffer as PNG.
pub(super) fn png(w: u32, h: u32, fill: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&fill(x, y));
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
    drop(writer);
    out
}

#[test]
fn decodes_modern_and_legacy_skins() {
    let modern = decode(&png(64, 64, |_, _| [1, 2, 3, 255])).unwrap();
    assert!(!modern.legacy);
    assert_eq!(modern.pixel(10, 10), [1, 2, 3, 255]);

    // Legacy: right leg at (0,16) is copied to the left leg slot (16,48).
    let legacy = decode(&png(64, 32, |x, y| {
        if (0..16).contains(&x) && (16..32).contains(&y) {
            [9, 9, 9, 255]
        } else {
            [0, 0, 0, 0]
        }
    }))
    .unwrap();
    assert!(legacy.legacy);
    assert_eq!(legacy.pixel(20, 52), [9, 9, 9, 255]);
}

#[test]
fn rejects_wrong_sizes() {
    assert!(decode(&png(32, 32, |_, _| [0; 4])).is_err());
    assert!(decode(&png(128, 128, |_, _| [0; 4])).is_err());
    assert!(decode(b"not a png").is_err());
}

#[test]
fn guesses_slim_arms() {
    let classic = decode(&png(64, 64, |_, _| [5, 5, 5, 255])).unwrap();
    assert_eq!(classic.guess_variant(), Variant::Classic);
    let slim = decode(&png(64, 64, |x, y| {
        if (54..56).contains(&x) && (16..32).contains(&y) {
            [0, 0, 0, 0]
        } else {
            [5, 5, 5, 255]
        }
    }))
    .unwrap();
    assert_eq!(slim.guess_variant(), Variant::Slim);
}

#[test]
fn library_add_update_remove() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::default();
    let bytes = png(64, 64, |_, _| [5, 5, 5, 255]);
    let entry = lib.add(dir.path(), "  Mine  ", &bytes, None).unwrap();
    assert_eq!(entry.name, "Mine");
    assert_eq!(entry.variant, Variant::Classic);
    assert_eq!(Library::read_png(dir.path(), &entry.id).unwrap(), bytes);

    lib.update(dir.path(), &entry.id, |e| e.variant = Variant::Slim)
        .unwrap();
    let loaded = Library::load(dir.path()).unwrap();
    assert_eq!(loaded.skins[0].variant, Variant::Slim);

    lib.remove(dir.path(), &entry.id).unwrap();
    assert!(Library::load(dir.path()).unwrap().skins.is_empty());
    assert!(!Library::png_path(dir.path(), &entry.id).exists());
}

#[test]
fn invalid_files_are_not_added() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::default();
    assert!(lib.add(dir.path(), "x", b"nope", None).is_err());
    assert!(lib.skins.is_empty());
}

#[test]
fn opaque_legacy_hat_is_cleared() {
    let skin = decode(&png(64, 32, |_, _| [0, 0, 0, 255])).unwrap();
    assert_eq!(skin.pixel(40, 8)[3], 0);
    assert_eq!(skin.pixel(8, 8)[3], 255);
    // A hat with any transparency is kept.
    let kept = decode(&png(64, 32, |x, y| {
        if x == 63 && y == 0 {
            [0; 4]
        } else {
            [1, 1, 1, 255]
        }
    }))
    .unwrap();
    assert_eq!(kept.pixel(40, 8)[3], 255);
}
