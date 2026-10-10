//! A page's link annotations, resolved to URIs and page numbers.

use hayro::hayro_interpret::TransformExt;
use hayro::hayro_syntax::object::{Array, Dict, MaybeRef, Name, ObjRef, Object};
use hayro::hayro_syntax::object::{Rect as PdfRect, String as PdfString};
use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::Pdf;
use hayro::kurbo::Rect;

use super::documents::{guarded, page_of};
use super::wire::{round, Link};

pub(super) fn page_links(pdf: &Pdf, page: u32) -> Result<Vec<Link>, String> {
    let page = page_of(pdf, page)?;
    guarded(|| collect_links(pdf, page))
}

/// The page's link annotations whose target resolves, boxed in rendered-page
/// space (rotation and crop-box origin applied).
fn collect_links(pdf: &Pdf, page: &Page<'_>) -> Vec<Link> {
    let Some(annots) = page.raw().get::<Array<'_>>(b"Annots") else {
        return Vec::new();
    };
    let view = page.initial_transform(true).to_kurbo();
    let mut links = Vec::new();
    for annot in annots.iter::<Dict<'_>>() {
        if !annot
            .get::<Name<'_>>(b"Subtype")
            .is_some_and(|kind| kind.as_ref() == b"Link")
        {
            continue;
        }
        let Some(rect) = annot.get::<PdfRect>(b"Rect") else {
            continue;
        };
        let (mut uri, mut target) = (None, None);
        if let Some(action) = annot.get::<Dict<'_>>(b"A") {
            let kind = action.get::<Name<'_>>(b"S");
            match kind.as_deref() {
                Some(b"URI") => {
                    uri = action
                        .get::<PdfString<'_>>(b"URI")
                        .map(|text| String::from_utf8_lossy(text.as_bytes()).into_owned());
                }
                Some(b"GoTo") => {
                    target = action
                        .get::<Object<'_>>(b"D")
                        .and_then(|dest| dest_page(pdf, dest));
                }
                _ => {}
            }
        } else if let Some(dest) = annot.get::<Object<'_>>(b"Dest") {
            target = dest_page(pdf, dest);
        }
        if uri.is_none() && target.is_none() {
            continue;
        }
        let bounds = view.transform_rect_bbox(Rect::new(rect.x0, rect.y0, rect.x1, rect.y1).abs());
        links.push(Link {
            x: round(bounds.x0),
            y: round(bounds.y0),
            width: round(bounds.width()),
            height: round(bounds.height()),
            uri,
            page: target,
        });
    }
    links
}

/// The 1-based page a destination points at: an explicit array, a dict with
/// `/D`, or a name looked up in the catalog.
fn dest_page(pdf: &Pdf, dest: Object<'_>) -> Option<u32> {
    match dest {
        Object::Array(array) => explicit_dest(pdf, &array),
        Object::Dict(dict) => match dict.get::<Object<'_>>(b"D")? {
            Object::Array(array) => explicit_dest(pdf, &array),
            _ => None,
        },
        Object::Name(name) => named_dest(pdf, name.as_ref()),
        Object::String(name) => named_dest(pdf, name.as_bytes()),
        _ => None,
    }
}

/// `[page /Fit …]`: the page is a reference to a page object, or (in some
/// producers) a 0-based index.
fn explicit_dest(pdf: &Pdf, dest: &Array<'_>) -> Option<u32> {
    let pages = pdf.pages();
    let index = match dest.raw_iter().next()? {
        MaybeRef::Ref(target) => pages
            .iter()
            .position(|page| page.raw().obj_id().map(ObjRef::from) == Some(target))?,
        MaybeRef::NotRef(Object::Number(number)) => usize::try_from(number.as_i64())
            .ok()
            .filter(|index| *index < pages.len())?,
        _ => return None,
    };
    u32::try_from(index + 1).ok()
}

/// A named destination: the catalog's `/Names /Dests` tree, or the older
/// `/Dests` dictionary.
fn named_dest(pdf: &Pdf, name: &[u8]) -> Option<u32> {
    let xref = pdf.xref();
    let catalog = xref.get::<Dict<'_>>(xref.root_id())?;
    let found = catalog
        .get::<Dict<'_>>(b"Names")
        .and_then(|names| names.get::<Dict<'_>>(b"Dests"))
        .and_then(|tree| name_tree_get(&tree, name, 0))
        .or_else(|| catalog.get::<Dict<'_>>(b"Dests")?.get::<Object<'_>>(name))?;
    match found {
        Object::Array(array) => explicit_dest(pdf, &array),
        Object::Dict(dict) => match dict.get::<Object<'_>>(b"D")? {
            Object::Array(array) => explicit_dest(pdf, &array),
            _ => None,
        },
        _ => None,
    }
}

/// A name tree's value for `key`, walking `/Kids` to a bounded depth so a
/// cyclic tree ends.
fn name_tree_get<'a>(node: &Dict<'a>, key: &[u8], depth: u8) -> Option<Object<'a>> {
    if depth > 32 {
        return None;
    }
    if let Some(names) = node.get::<Array<'a>>(b"Names") {
        let mut items = names.iter::<Object<'a>>();
        while let (Some(name), Some(value)) = (items.next(), items.next()) {
            if matches!(&name, Object::String(name) if name.as_bytes() == key) {
                return Some(value);
            }
        }
    }
    node.get::<Array<'a>>(b"Kids")?
        .iter::<Dict<'a>>()
        .find_map(|kid| name_tree_get(&kid, key, depth + 1))
}
