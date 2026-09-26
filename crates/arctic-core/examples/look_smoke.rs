//! Publish a look for the active account of a data dir, the way the
//! launcher does: `look_smoke <data-dir> <skin.png> [slim|classic] [cape preset]`
//! (set ARCTIC_COSMETICS_URL to a local server).
use arctic_core::auth::AccountStore;
use arctic_core::cosmetics::{self, CapeChoice, NewLook, Texture};
use arctic_core::skins::Variant;
use arctic_core::storage::DataDirs;

fn main() {
    let mut args = std::env::args().skip(1);
    let root = DataDirs::new(args.next().expect("data dir"));
    let dirs = root.with_profile("default");
    let png = std::fs::read(args.next().expect("skin png")).unwrap();
    let variant = if args.next().as_deref() == Some("slim") {
        Variant::Slim
    } else {
        Variant::Classic
    };
    let cape = args.next().unwrap_or_else(|| "aurora".into());
    let account = AccountStore::load(&dirs)
        .unwrap()
        .active()
        .cloned()
        .expect("an account");
    let base = cosmetics::base_url();
    let token = cosmetics::token_for(&dirs, &base, &account).unwrap();
    let again = cosmetics::token_for(&dirs, &base, &account).unwrap();
    assert_eq!(token, again, "token is reused");
    let look = cosmetics::set_look(
        &base,
        &token,
        &NewLook {
            skin: Some((Texture::Png(png), variant)),
            cape: Some(CapeChoice::Preset(cape.clone())),
            cosmetics: None,
        },
    )
    .unwrap();
    println!(
        "published for {} ({}): {look:?}",
        account.username, account.uuid
    );
    // Change only the cape, re-publishing the skin by hash.
    let presets = cosmetics::catalog(&base).unwrap();
    let mut next = NewLook::from_look(&look, &presets);
    next.cape = Some(CapeChoice::Preset("glacier".into()));
    let look = cosmetics::set_look(&base, &token, &next).unwrap();
    println!("cape changed: {look:?}");
    next = NewLook::from_look(&look, &presets);
    // Finish on the chosen cape.
    next.cape = Some(CapeChoice::Preset(cape));
    cosmetics::set_look(&base, &token, &next).unwrap();
}
