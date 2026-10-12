//! A `starter:` src is a bundled image, with or without an assets folder;
//! an unknown one names the ones there are.

use super::{BUNDLED, ImageCache};

#[test]
fn a_bundled_image_needs_no_assets_folder() {
    let mut cache = ImageCache::new(None);
    for (src, _) in BUNDLED {
        let image = cache.get(src).unwrap();
        assert!(image.width() > 100 && image.height() > 100, "{src}");
    }
    let err = cache.get("starter:nothing.png").unwrap_err();
    assert!(err.contains("not a bundled image"), "{err}");
    assert!(err.contains("starter:slide-figure.png"), "{err}");
    let err = cache.get("slides/fig.png").unwrap_err();
    assert!(err.contains("no assets folder"), "{err}");
}
