//! Live check of the public skin lookup:
//! `cargo run -p arctic-core --example skin_lookup -- Notch [library-dir]`
//! With a library folder, the skin is also added to that library.
fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "Notch".into());
    let library = args.next().map(std::path::PathBuf::from);
    match arctic_core::skins::api::player_skin(&name) {
        Ok((png, variant)) => {
            let image = arctic_core::skins::decode(&png).expect("decodes");
            println!(
                "{name}: {} bytes, {variant:?}, legacy={}, guessed {:?}",
                png.len(),
                image.legacy,
                image.guess_variant()
            );
            if let Some(dir) = library {
                let mut lib = arctic_core::skins::Library::load(&dir).expect("library");
                lib.add(&dir, &name, &png, Some(variant)).expect("added");
            }
        }
        Err(e) => println!("{name}: {e}"),
    }
}
