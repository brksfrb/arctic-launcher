//! Background jobs for the skin gallery.

use arctic_core::auth::Account;
use arctic_core::cosmetics::{self, GalleryItem, GalleryPage, GallerySort};
use arctic_core::skins::Variant;

use crate::tasks::{Event, Tasks};

/// A gallery page with every skin's texture.
#[derive(Debug, Clone, Default)]
pub struct GalleryResult {
    pub page: GalleryPage,
    /// (texture hash, PNG)
    pub textures: Vec<(String, Vec<u8>)>,
}

impl Tasks {
    pub fn gallery_search(&self, request: u64, sort: GallerySort, query: String) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let result = cosmetics::gallery(&base, sort, &query, 0)
                .map(|page| {
                    let textures = page
                        .items
                        .iter()
                        .filter_map(|i| {
                            cosmetics::texture(&base, &i.texture)
                                .ok()
                                .map(|png| (i.texture.clone(), png))
                        })
                        .collect();
                    GalleryResult { page, textures }
                })
                .map_err(|e| friendly(&e.to_string()));
            t.send(Event::Gallery(request, result));
        });
    }

    /// Download a gallery skin (counting the use); `wear` also puts it on.
    pub fn gallery_take(&self, item: GalleryItem, wear: bool) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let result = cosmetics::gallery_use(&base, &item.id)
                .and_then(|hash| cosmetics::texture(&base, &hash))
                .map(|png| (item.name.clone(), png, item.variant(), wear))
                .map_err(|e| friendly(&e.to_string()));
            t.send(Event::GalleryTaken(result));
        });
    }

    pub fn gallery_share(&self, account: Account, png: Vec<u8>, variant: Variant, name: String) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let result = cosmetics::token_for(t.dirs(), &base, &account)
                .and_then(|token| cosmetics::gallery_share(&base, &token, &png, variant, &name))
                .map(|()| format!("Shared {name}"))
                .map_err(|e| friendly(&e.to_string()));
            t.send(Event::GalleryDone(result));
        });
    }

    pub fn gallery_report(&self, account: Account, id: String) {
        self.run(move |t| {
            let base = cosmetics::base_url();
            let result = cosmetics::token_for(t.dirs(), &base, &account)
                .and_then(|token| cosmetics::gallery_report(&base, &token, &id))
                .map(|()| "Reported. Thanks for keeping the gallery clean.".to_owned())
                .map_err(|e| friendly(&e.to_string()));
            t.send(Event::GalleryDone(result));
        });
    }
}

fn friendly(e: &str) -> String {
    if e.starts_with("network error") {
        log::info!("gallery: {e}");
        "Can't reach the Arctic gallery right now.".into()
    } else {
        e.to_owned()
    }
}
