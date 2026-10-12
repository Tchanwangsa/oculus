//! Which of the jar's cookies are the university's session, for capturing
//! them into keyd, seeding them back and clearing them at sign-out. A cookie
//! set on the host itself is. One on a parent domain is only when that domain
//! is the university's own (`institution::institution_domain`): a parent such
//! as `.google.com` or `.instructure.com` carries the student's other
//! sessions, which the app must not copy, shadow with a host copy or delete.

use super::seed::sent_to;

/// Whether a cookie scoped to `domain` was set on `host` itself (host-only, or
/// a domain cookie naming exactly that host).
pub(crate) fn on_host(domain: &str, host: &str) -> bool {
    domain.trim_start_matches('.').eq_ignore_ascii_case(host)
}

/// Whether a cookie scoped to `domain` reaches `host` from a parent domain.
pub(crate) fn at_parent(domain: &str, host: &str) -> bool {
    !on_host(domain, host) && sent_to(domain, host)
}

/// Whether a cookie scoped to `domain`, sent to `host`, is the university's
/// session there: set on the host, or on a parent inside `university`.
pub(crate) fn belongs(domain: &str, host: &str, university: Option<&str>) -> bool {
    on_host(domain, host)
        || (at_parent(domain, host)
            && university.is_some_and(|u| sent_to(u, domain.trim_start_matches('.'))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_parent_domain_belongs_only_when_it_is_the_universitys() {
        let uni = Some("unimelb.edu.au");
        assert!(belongs("sso.unimelb.edu.au", "sso.unimelb.edu.au", uni));
        assert!(belongs(".sso.unimelb.edu.au", "sso.unimelb.edu.au", None));
        assert!(belongs(".unimelb.edu.au", "sso.unimelb.edu.au", uni));
        assert!(!belongs(".unimelb.edu.au", "sso.unimelb.edu.au", None));
        assert!(!belongs(".edu.au", "sso.unimelb.edu.au", uni));
        assert!(!belongs(".google.com", "accounts.google.com", None));
        assert!(!belongs(".instructure.com", "rmit.instructure.com", None));
        assert!(!belongs("other.unimelb.edu.au", "sso.unimelb.edu.au", uni));
    }

    #[test]
    fn at_parent_is_a_wider_domain_that_still_reaches_the_host() {
        assert!(at_parent(".google.com", "accounts.google.com"));
        assert!(!at_parent("accounts.google.com", "accounts.google.com"));
        assert!(!at_parent(".mail.google.com", "accounts.google.com"));
    }
}
