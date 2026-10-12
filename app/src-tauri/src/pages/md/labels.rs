//! The provider label of an embed rendered as a link (`[H5P: title](src)`).

use url::Url;

/// Host suffix → label, first match wins.
const HOSTS: &[(&str, &str)] = &[
    ("h5p.com", "H5P"),
    ("padlet.com", "Padlet"),
    ("padlet.org", "Padlet"),
    ("qualtrics.com", "Survey"),
    ("q.surveys.unimelb.edu.au", "Survey"),
    ("mentimeter.com", "Mentimeter"),
    ("menti.com", "Mentimeter"),
    ("sharepoint.com", "SharePoint"),
    ("kaltura.com", "Kaltura"),
    ("youtube.com", "YouTube"),
    ("youtube-nocookie.com", "YouTube"),
    ("youtu.be", "YouTube"),
    ("vimeo.com", "Vimeo"),
];

/// `host` is `domain` or one of its subdomains.
pub(super) fn host_is(host: &str, domain: &str) -> bool {
    host.strip_suffix(domain)
        .is_some_and(|rest| rest.is_empty() || rest.ends_with('.'))
}

/// What an embed is, from its URL: a Canvas LTI launch by the tool it wraps,
/// a known provider by host, anything else by its host without `www.`.
/// `title` names a Readings Online launch, whose URL doesn't.
pub(super) fn embed_label(url: &Url, title: &str) -> String {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = url.path();

    if path.ends_with("/external_tools/retrieve") {
        let inner = url
            .query_pairs()
            .find(|(k, _)| k == "url")
            .and_then(|(_, v)| Url::parse(&v).ok());
        let label = match inner.as_ref().and_then(Url::host_str) {
            Some(h) if host_is(h, "h5p.com") => "H5P",
            Some(h) if host_is(h, "kaltura.com") => "Kaltura",
            None if title.starts_with("ERES_") => "Readings Online",
            _ => "Canvas tool",
        };
        return label.to_string();
    }
    if path.contains("/media_objects_iframe/") {
        return "Canvas media".to_string();
    }
    if host == "docs.google.com" {
        let label = if path.starts_with("/forms") {
            "Google Form"
        } else {
            "Google Docs"
        };
        return label.to_string();
    }
    if host_is(&host, "echo360.net.au") && path.starts_with("/media/") && path.ends_with("/public")
    {
        return "Echo360".to_string();
    }
    if let Some((_, label)) = HOSTS.iter().find(|(d, _)| host_is(&host, d)) {
        return label.to_string();
    }
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}
