//! Share every skin in a data dir's library to the gallery (dev seeding):
//! `ARCTIC_COSMETICS_URL=... cargo run -p arctic-core --example gallery_seed -- <data-dir>`
use arctic_core::auth::AccountStore;
use arctic_core::cosmetics;
use arctic_core::skins::Library;
use arctic_core::storage::DataDirs;

fn main() {
    let root = DataDirs::new(std::env::args().nth(1).expect("data dir"));
    let dirs = root.with_profile("default");
    let account = AccountStore::load(&dirs)
        .unwrap()
        .active()
        .cloned()
        .expect("account");
    let base = cosmetics::base_url();
    let token = cosmetics::token_for(&dirs, &base, &account).unwrap();
    let dir = Library::dir(dirs.profile_root());
    for entry in Library::load(&dir).unwrap().skins {
        let png = Library::read_png(&dir, &entry.id).unwrap();
        match cosmetics::gallery_share(&base, &token, &png, entry.variant, &entry.name) {
            Ok(()) => println!("shared {}", entry.name),
            Err(e) => println!("{}: {e}", entry.name),
        }
    }
    let page = cosmetics::gallery(&base, cosmetics::GallerySort::Popular, "", 0).unwrap();
    println!("gallery has {} skins", page.total);
}
