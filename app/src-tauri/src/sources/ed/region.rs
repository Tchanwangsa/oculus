//! Ed's regions. A launch lands on `edstem.org/<region>/courses/…`, and the
//! region decides the API host: the home region's API is on the bare host,
//! every other region's on `<region>.edstem.org`.

/// Ed's bare host.
const HOST: &str = "edstem.org";
/// The region whose API is on the bare host.
const HOME_REGION: &str = "au";

/// Whether `host` is Ed's: the bare host or one of its subdomains.
pub(super) fn is_ed_host(host: &str) -> bool {
    host == HOST
        || host
            .strip_suffix(HOST)
            .is_some_and(|rest| rest.ends_with('.'))
}

fn is_region_code(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_lowercase())
}

/// The region a launch landed in: the first path segment
/// (`https://edstem.org/au/courses/1` → `au`), else a regional subdomain.
pub(super) fn region_of_landing(landed: &url::Url) -> Option<String> {
    let mut segments = landed.path_segments()?;
    let first = segments.next()?;
    if segments.next() == Some("courses") && is_region_code(first) {
        return Some(first.to_string());
    }
    subdomain_region(landed.host_str()?)
}

/// `<region>.edstem.org` → the region.
fn subdomain_region(host: &str) -> Option<String> {
    host.strip_suffix(HOST)?
        .strip_suffix('.')
        .filter(|r| is_region_code(r))
        .map(str::to_string)
}

/// Whether `api` is on one of Ed's hosts.
pub(super) fn is_ed_api(api: &str) -> bool {
    url::Url::parse(api)
        .ok()
        .and_then(|u| u.host_str().map(is_ed_host))
        .unwrap_or(false)
}

/// The API base that serves `region`.
pub(super) fn api_for_region(region: &str) -> String {
    if region == HOME_REGION {
        format!("https://{HOST}/api")
    } else {
        format!("https://{region}.{HOST}/api")
    }
}

/// The region an API base serves, the inverse of [`api_for_region`]; `None`
/// for a host that is not Ed's.
pub(super) fn region_of_api(api: &str) -> Option<String> {
    let url = url::Url::parse(api).ok()?;
    let host = url.host_str()?;
    if host == HOST {
        return Some(HOME_REGION.to_string());
    }
    subdomain_region(host)
}
