use super::layout::{parked_rect, slot};
use super::services::{service, store_id, uuid_string, SERVICES};
use super::*;
use tauri::LogicalSize;

/// A changed store id signs the user out of that service for good, so the
/// ids are pinned here (they match the stage 0 spike's stores).
#[test]
fn store_ids_never_change() {
    let ids: Vec<String> = SERVICES
        .iter()
        .map(|s| format!("{} {}", s.id, uuid_string(&s.store_id)))
        .collect();
    assert_eq!(
        ids,
        [
            "whatsapp 873D0236-0E30-F207-CB52-AE69B1E6F21A",
            "instagram B91676AB-1597-5F7B-9818-12CE1AE92010",
            "messenger F24418B3-AB8B-A404-D975-CE969A9D2813",
            "discord 42071853-4183-B90F-097A-584B456A36D4",
        ]
    );
    for s in &SERVICES {
        assert_eq!(s.store_id, store_id(s.id));
    }
}

#[test]
fn services_are_https_and_have_sites() {
    for s in &SERVICES {
        let home: url::Url = s.home.parse().unwrap();
        assert_eq!(home.scheme(), "https", "{}", s.id);
        assert!(!s.sites.is_empty(), "{}", s.id);
        for site in s.sites {
            let site: url::Url = site.parse().unwrap();
            assert_eq!(site.scheme(), "https", "{}", s.id);
        }
        assert_eq!(service(s.id).map(|f| f.id), Some(s.id));
    }
    assert!(service("slack").is_none());
    assert_eq!(label("discord"), "dock-discord");
}

/// The page's own origin must pass the `dock` channel's check, or nothing
/// it reports would count.
#[test]
fn every_services_home_and_sites_pass_the_origin_check() {
    for s in &SERVICES {
        for url in std::iter::once(s.home).chain(s.sites.iter().copied()) {
            let url: url::Url = url.parse().unwrap();
            let host = url.host_str().unwrap();
            assert!(
                super::channel::origin_allowed(s, url.scheme(), host),
                "{} {url}",
                s.id
            );
        }
    }
}

#[test]
fn a_parked_page_keeps_the_slot_size_off_screen() {
    let window = LogicalSize::new(1480.0, 920.0);
    let vp = Viewport {
        left: 260.0,
        top: 52.0,
        right: 8.0,
        bottom: 8.0,
        radius: 12.0,
    };
    let (position, size) = parked_rect(window, Some(vp));
    assert_eq!((size.width, size.height), (1212.0, 860.0));
    assert!(position.x + size.width < 0.0, "entirely left of the window");
    assert_eq!(position.y, 52.0);

    let (position, size) = parked_rect(window, None);
    assert_eq!((size.width, size.height), (1480.0, 920.0));
    assert!(position.x + size.width < 0.0);
}

#[test]
fn an_unplaced_page_parks_at_the_latest_slot() {
    let vp = |left| Viewport {
        left,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        radius: 0.0,
    };
    let mut s = Inner::default();
    assert!(slot(&s, "discord").is_none());
    s.last_viewport = Some(vp(10.0));
    assert_eq!(slot(&s, "discord").map(|v| v.left), Some(10.0));
    s.viewports.insert("discord".into(), vp(20.0));
    assert_eq!(slot(&s, "discord").map(|v| v.left), Some(20.0));
}

#[test]
fn the_snapshot_is_camel_case() {
    let snap = DockSnapshot {
        pages: vec![DockPageInfo {
            id: "whatsapp".into(),
            title: "(2) WhatsApp".into(),
            url: "https://web.whatsapp.com/".into(),
            loading: false,
            unread: 2,
        }],
        shown: Some("whatsapp".into()),
    };
    assert_eq!(
        serde_json::to_value(&snap).unwrap(),
        serde_json::json!({
            "pages": [{
                "id": "whatsapp",
                "title": "(2) WhatsApp",
                "url": "https://web.whatsapp.com/",
                "loading": false,
                "unread": 2,
            }],
            "shown": "whatsapp",
        })
    );
}
