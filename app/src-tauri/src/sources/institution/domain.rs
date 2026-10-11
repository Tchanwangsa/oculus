//! What the Canvas and SSO hosts say about the university: the domain they
//! share, and which hemisphere it is in.

/// Country codes south of the equator, for reading season-named terms.
const SOUTHERN_TLDS: &[&str] = &["au", "nz", "za", "ar", "br", "cl", "uy"];

pub(super) fn is_southern(canvas: &str, sso_host: Option<&str>) -> bool {
    super::host_of(canvas)
        .into_iter()
        .chain(sso_host.map(str::to_string))
        .any(|h| {
            h.rsplit('.')
                .next()
                .is_some_and(|tld| SOUTHERN_TLDS.contains(&tld))
        })
}

/// Second-level labels under a country code that are registries, not
/// institutions: `edu.au`, `ac.uk`, `co.nz`.
const REGISTRY_LABELS: &[&str] = &["ac", "co", "com", "edu", "gov", "net", "org", "sch"];

/// Hosting providers whose subdomains belong to different customers.
const SHARED_HOSTS: &[&str] = &["instructure.com", "okta.com", "oktapreview.com"];

/// The domain the Canvas and SSO hosts share, when it is the university's own
/// (`unimelb.edu.au`), not a public suffix (`edu.au`) or a shared host
/// (`instructure.com`). The in-app browser seeds sessions for pages inside it.
pub fn institution_domain(canvas_host: &str, sso_host: Option<&str>) -> Option<String> {
    let sso = sso_host?;
    let a: Vec<&str> = canvas_host.rsplit('.').collect();
    let b: Vec<&str> = sso.rsplit('.').collect();
    let shared: Vec<&str> = a
        .iter()
        .zip(b.iter())
        .take_while(|(x, y)| x.eq_ignore_ascii_case(y))
        .map(|(x, _)| *x)
        .collect();
    let minimum = match shared.as_slice() {
        [tld, second, ..] if tld.len() == 2 && REGISTRY_LABELS.contains(second) => 3,
        _ => 2,
    };
    if shared.len() < minimum {
        return None;
    }
    let domain = shared.iter().rev().copied().collect::<Vec<_>>().join(".");
    (!SHARED_HOSTS.contains(&domain.as_str())).then_some(domain)
}
