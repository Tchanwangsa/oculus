use super::client::{get, post, request};
use super::{add, document, image_document, shape, Harness, FILE, LAN};
use crate::companion::limits::Limits;
use serde_json::json;

fn commit(
    shared: &super::Shared,
    client: &str,
    seq: u64,
    transaction: serde_json::Value,
) -> serde_json::Value {
    json!({"clientId": client, "seq": seq, "opId": format!("{client}-{seq}"),
        "epoch": shared.info.epoch, "transaction": transaction})
}

#[tokio::test]
async fn share_endpoints_need_the_key_and_never_echo_it() {
    let h = Harness::new("auth");
    h.open(FILE, &document(vec![shape("a", 0.0)]));
    let s = h.share(FILE);
    let path = s.path("snapshot");
    let wrong = format!("Bearer {}.{}", s.id(), "x".repeat(43));
    let other = format!("Bearer {}.{}", "0".repeat(32), s.secret());
    let cases = [
        (vec![], 401),
        (vec![("Authorization", wrong.as_str())], 403),
        (vec![("Authorization", other.as_str())], 403),
        (vec![("Authorization", "Bearer nodot")], 403),
        (vec![("Authorization", "Basic abc")], 401),
    ];
    for (headers, status) in cases {
        let reply = get(s.port, &path, &headers).await;
        assert_eq!(reply.status, status, "{headers:?}: {}", reply.text());
        assert!(!reply.everything().contains(s.secret()));
    }
    let bearer = s.bearer();
    let ok = get(s.port, &path, &[("Authorization", &bearer)]).await;
    assert_eq!(ok.status, 200);
    assert!(!ok.everything().contains(s.secret()));
}

#[tokio::test]
async fn host_and_origin_must_name_this_server() {
    let h = Harness::new("host");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let bearer = s.bearer();
    let lan = format!("{LAN}:{}", s.port);
    let local = format!("localhost:{}", s.port);
    let lan_origin = format!("http://{lan}");
    let wrong_port = format!("127.0.0.1:{}", s.port.wrapping_add(1));
    let path = s.path("snapshot");
    let refused = [
        vec![("Host", "evil.example")],
        vec![("Host", "evil.example:80")],
        vec![("Host", "127.0.0.1")],
        vec![("Host", wrong_port.as_str())],
        vec![("Host", "192.0.2.99:1")],
        vec![("Origin", "http://evil.example")],
        vec![("Origin", "null")],
        vec![("Host", lan.as_str()), ("Origin", "http://127.0.0.1")],
    ];
    for headers in refused {
        let mut headers = headers.clone();
        headers.push(("Authorization", &bearer));
        let reply = get(s.port, &path, &headers).await;
        assert_eq!(reply.status, 403, "{headers:?}");
        assert_eq!(reply.header("connection"), Some("close"));
    }
    for headers in [
        vec![("Host", lan.as_str()), ("Origin", lan_origin.as_str())],
        vec![("Host", local.as_str())],
    ] {
        let mut headers = headers.clone();
        headers.push(("Authorization", &bearer));
        assert_eq!(
            get(s.port, &path, &headers).await.status,
            200,
            "{headers:?}"
        );
    }
    let put = request(s.port, "PUT", &path, &[("Authorization", &bearer)], b"{}").await;
    assert_eq!(put.status, 405);
    assert_eq!(put.header("connection"), Some("close"));
    let options = request(s.port, "OPTIONS", &path, &[], b"").await;
    assert_eq!(options.status, 405);
    assert_eq!(get(s.port, "/api/other", &[]).await.status, 404);
    assert_eq!(get(s.port, "/", &[]).await.status, 404);
}

#[tokio::test]
async fn a_large_canvas_snapshot_arrives_whole_with_its_exact_length() {
    let h = Harness::new("snapshot");
    let rel = h.open(FILE, &image_document(10 * 1024 * 1024));
    let disk = std::fs::read(h.root.join(&rel)).unwrap();
    assert!(disk.len() > 10 * 1024 * 1024);
    let s = h.share(FILE);
    let reply = get(
        s.port,
        &s.path("snapshot"),
        &[("Authorization", &s.bearer())],
    )
    .await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.header("content-length"),
        Some(disk.len().to_string().as_str())
    );
    assert!(reply.header("transfer-encoding").is_none());
    assert_eq!(reply.body.len(), disk.len());
    assert!(reply.body == disk, "the snapshot is the file's bytes");
    assert_eq!(reply.header("x-oculus-epoch"), Some(s.info.epoch.as_str()));
    assert_eq!(reply.header("x-oculus-revision"), Some("0"));
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    assert_eq!(reply.header("referrer-policy"), Some("no-referrer"));
}

#[tokio::test]
async fn a_retried_commit_gets_its_original_ack() {
    let h = Harness::new("idempotent");
    let rel = h.open(FILE, &document(vec![shape("a", 0.0)]));
    let s = h.share(FILE);
    let body = commit(&s, "tab", 1, add("b"));
    let first = post(s.port, &s.path("commit"), &s.bearer(), &body).await;
    assert_eq!(first.status, 200, "{}", first.text());
    let ack = first.json();
    assert_eq!(ack["revision"], 1);
    assert_eq!(ack["replayed"], false);
    assert_eq!(ack["seq"], 1);
    assert_eq!(ack["opId"], "tab-1");
    assert_eq!(ack["epoch"], s.info.epoch.as_str());
    assert!(ack["transaction"].is_object());
    // The response was "lost": the client sends the same op again.
    let retry = post(s.port, &s.path("commit"), &s.bearer(), &body)
        .await
        .json();
    assert_eq!(retry["revision"], 1);
    assert_eq!(retry["replayed"], true);
    assert!(retry["transaction"].is_null());
    let disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(h.root.join(&rel)).unwrap()).unwrap();
    assert_eq!(disk["elements"].as_array().unwrap().len(), 2);
    let changes = get(
        s.port,
        &format!("{}?from=0", s.path("changes")),
        &[("Authorization", &s.bearer())],
    )
    .await;
    let changes = changes.json();
    assert_eq!(changes.as_array().unwrap().len(), 1, "{changes}");
    assert_eq!(changes[0]["clientId"], "web:tab");
    assert_eq!(changes[0]["revision"], 1);
    assert_eq!(changes[0]["epoch"], s.info.epoch.as_str());
}

#[tokio::test]
async fn two_devices_merge_and_a_real_conflict_is_409() {
    let h = Harness::new("conflict");
    h.open(FILE, &document(vec![shape("a", 0.0)]));
    let s = h.share(FILE);
    let path = s.path("commit");
    let a = post(s.port, &path, &s.bearer(), &commit(&s, "a", 1, add("x"))).await;
    let b = post(s.port, &path, &s.bearer(), &commit(&s, "b", 1, add("y"))).await;
    assert_eq!(
        (a.json()["revision"].clone(), b.json()["revision"].clone()),
        (json!(1), json!(2))
    );
    let moved =
        |x: f64| json!({"edits": [{"id": "a", "before": shape("a", 0.0), "after": shape("a", x)}]});
    let first = post(s.port, &path, &s.bearer(), &commit(&s, "a", 2, moved(10.0))).await;
    assert_eq!(first.status, 200);
    let second = post(s.port, &path, &s.bearer(), &commit(&s, "b", 2, moved(20.0))).await;
    assert_eq!(second.status, 409, "{}", second.text());
    assert_eq!(second.json()["code"], "conflict-shape");
    let stale_epoch =
        json!({"clientId": "c", "seq": 1, "opId": "c-1", "epoch": "old", "transaction": add("z")});
    let ended = post(s.port, &path, &s.bearer(), &stale_epoch).await;
    assert_eq!(
        (ended.status, ended.json()["code"].clone()),
        (410, json!("session-ended"))
    );
    let garbage = request(
        s.port,
        "POST",
        &path,
        &[("Authorization", &s.bearer())],
        b"{nope",
    )
    .await;
    assert_eq!(
        (garbage.status, garbage.json()["code"].clone()),
        (400, json!("invalid"))
    );
    let no_epoch = json!({"clientId": "c", "seq": 1, "opId": "c-1", "transaction": add("z")});
    assert_eq!(
        post(s.port, &path, &s.bearer(), &no_epoch).await.status,
        400
    );
}

#[tokio::test]
async fn an_outside_edit_pauses_commits_with_423() {
    let h = Harness::new("external");
    let rel = h.open(FILE, &document(vec![shape("a", 0.0)]));
    let s = h.share(FILE);
    std::fs::write(
        h.root.join(&rel),
        crate::library::files::canvases::authority::format::serialize(&document(vec![])).unwrap(),
    )
    .unwrap();
    let reply = post(
        s.port,
        &s.path("commit"),
        &s.bearer(),
        &commit(&s, "a", 1, add("x")),
    )
    .await;
    assert_eq!(
        (reply.status, reply.json()["code"].clone()),
        (423, json!("external-edit"))
    );
}

#[tokio::test]
async fn catch_up_comes_from_the_log_until_it_is_evicted() {
    let h = Harness::new("changes");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    for seq in 1..=3 {
        let reply = post(
            s.port,
            &s.path("commit"),
            &s.bearer(),
            &commit(&s, "t", seq, add(&format!("e{seq}"))),
        )
        .await;
        assert_eq!(reply.status, 200);
    }
    let auth = [("Authorization", s.bearer())];
    let auth: Vec<(&str, &str)> = auth.iter().map(|(n, v)| (*n, v.as_str())).collect();
    let changes = |query: &str| format!("{}?{query}", s.path("changes"));
    let two = get(s.port, &changes("from=1"), &auth).await;
    assert_eq!(two.status, 200);
    assert_eq!(two.header("x-oculus-revision"), Some("3"));
    let revisions: Vec<u64> = two
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["revision"].as_u64().unwrap())
        .collect();
    assert_eq!(revisions, vec![2, 3]);
    let with_epoch = changes(&format!("from=3&epoch={}", s.info.epoch));
    assert_eq!(get(s.port, &with_epoch, &auth).await.json(), json!([]));
    for query in ["from=4", "from=1&epoch=other"] {
        let gone = get(s.port, &changes(query), &auth).await;
        assert_eq!(
            (gone.status, gone.json()["code"].clone()),
            (410, json!("gone")),
            "{query}"
        );
    }
    assert_eq!(get(s.port, &changes("since=1"), &auth).await.status, 400);
    for seq in 1..=260 {
        h.desktop_commit(FILE, seq, add(&format!("d{seq}")));
    }
    assert_eq!(get(s.port, &changes("from=3"), &auth).await.status, 410);
    let last = get(s.port, &changes("from=262"), &auth).await.json();
    assert_eq!(last.as_array().unwrap().len(), 1);
    assert_eq!(last[0]["clientId"], "desk");
    assert_eq!(last[0]["revision"], 263);
}

#[tokio::test]
async fn oversized_commits_are_refused_before_they_are_read() {
    assert_eq!(Limits::default().commit_bytes, 80 * 1024 * 1024);
    let limits = Limits {
        commit_bytes: 1024 * 1024,
        ..Limits::default()
    };
    let h = Harness::with("too-large", limits, 0);
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let bearer = s.bearer();
    let declared = (1024 * 1024 + 1).to_string();
    let reply = request(
        s.port,
        "POST",
        &s.path("commit"),
        &[("Authorization", &bearer), ("Content-Length", &declared)],
        b"",
    )
    .await;
    assert_eq!(
        (reply.status, reply.json()["code"].clone()),
        (413, json!("too-large"))
    );
    let mut chunked = Vec::new();
    for _ in 0..3 {
        let chunk = vec![b' '; 512 * 1024];
        chunked.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        chunked.extend_from_slice(&chunk);
        chunked.extend_from_slice(b"\r\n");
    }
    chunked.extend_from_slice(b"0\r\n\r\n");
    let reply = request(
        s.port,
        "POST",
        &s.path("commit"),
        &[("Authorization", &bearer), ("Transfer-Encoding", "chunked")],
        &chunked,
    )
    .await;
    assert_eq!(reply.status, 413);
}

#[tokio::test]
async fn the_bundle_is_served_from_its_allow_list_only() {
    let h = Harness::new("assets");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let page = get(s.port, "/c/", &[]).await;
    assert_eq!(page.status, 200);
    assert_eq!(
        page.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(page.header("cache-control"), Some("no-store"));
    assert_eq!(page.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(page.header("referrer-policy"), Some("no-referrer"));
    let wasm = get(s.port, "/c/assets/geometry.wasm", &[]).await;
    assert_eq!(wasm.status, 200);
    assert_eq!(wasm.header("content-type"), Some("application/wasm"));
    assert_eq!(wasm.body, b"\0asm\x01\0\0\0");
    let redirect = get(s.port, "/c", &[]).await;
    assert_eq!(
        (redirect.status, redirect.header("location")),
        (308, Some("/c/"))
    );
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        h.root.join("outside.js"),
        h.root.join("dist/companion/assets/link.js"),
    )
    .unwrap();
    for path in [
        "/c/assets/../../outside.js",
        "/c/assets/%2e%2e/%2e%2e/outside.js",
        "/c/assets/..%2f..%2foutside.js",
        "/c/assets/a%00.js",
        "/c/assets/link.js",
        "/c/../outside.js",
        "/c/outside.js",
        "/c/assets/",
        "/c/assets//geometry.wasm",
    ] {
        let reply = get(s.port, path, &[]).await;
        assert!(
            matches!(reply.status, 400 | 404),
            "{path}: {}",
            reply.status
        );
        assert!(!reply.text().contains("outside the bundle"), "{path}");
    }
    assert_eq!(request(s.port, "POST", "/c/", &[], b"").await.status, 405);
    std::fs::remove_dir_all(h.root.join("dist")).unwrap();
    let missing = get(s.port, "/c/", &[]).await;
    assert_eq!(missing.status, 404);
    assert!(
        missing.text().contains("bun run companion:build"),
        "{}",
        missing.text()
    );
}
