//! Unread counts from page titles. All four services lead the title with
//! `(N)` while something is unread: `(2) WhatsApp`, `(356) Discord | Friends`,
//! `(4) Instagram • Messages`, `(20+) Messenger | Facebook`. WhatsApp and
//! Instagram count chats, Discord messages, and Facebook everything.

/// The leading `(N)` or `(N+)` of `title` as N; 0 when there is none.
/// Saturates rather than failing on an absurd number.
pub(super) fn unread(title: &str) -> u32 {
    let Some(rest) = title.trim_start().strip_prefix('(') else {
        return 0;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return 0;
    }
    let tail = &rest[digits..];
    let tail = tail.strip_prefix('+').unwrap_or(tail);
    if !tail.starts_with(')') {
        return 0;
    }
    rest[..digits].bytes().fold(0u32, |n, d| {
        n.saturating_mul(10).saturating_add(u32::from(d - b'0'))
    })
}

#[cfg(test)]
mod tests {
    use super::unread;

    #[test]
    fn reads_each_services_format() {
        assert_eq!(unread("(2) WhatsApp"), 2);
        assert_eq!(unread("(356) Discord | Friends"), 356);
        assert_eq!(unread("(4) Instagram • Messages"), 4);
        assert_eq!(unread("(20+) Messenger | Facebook"), 20);
    }

    #[test]
    fn no_count_is_zero() {
        for title in [
            "",
            "WhatsApp",
            "Discord | Friends",
            "Instagram",
            "Messenger | Facebook",
            "()",
            "(+) x",
            "(4 Instagram",
            "(4]) x",
            "(a) x",
            "Chat (3) x",
            "(-1) x",
            "(1.5) x",
        ] {
            assert_eq!(unread(title), 0, "{title:?}");
        }
    }

    #[test]
    fn tolerates_spacing_and_saturates() {
        assert_eq!(unread("  (7) WhatsApp"), 7);
        assert_eq!(unread("(0) WhatsApp"), 0);
        assert_eq!(unread("(99999999999999) x"), u32::MAX);
    }
}
