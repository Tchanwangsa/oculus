//! Telling oculus-keyd which university's origins to send the session to
//! (`keyd_core::site`). keyd drops what it held for an origin that moved, so
//! it hears before the profile is saved: if it refuses, nothing changes.

use std::path::Path;

use super::Institution;
use crate::providers::credentials::{Credentialed, KeydError};

/// With keyd absent there is no session anywhere to send, so nothing to tell.
pub(super) fn tell(data_dir: &Path, inst: &Institution) -> Result<(), String> {
    match Credentialed::at(data_dir).site_set(&inst.site()) {
        Ok(change) => {
            if !change.cleared.is_empty() || change.forgot_okta {
                elog!(
                    "[oculus] oculus-keyd moved to {}: dropped {} session(s){}",
                    inst.canvas,
                    change.cleared.len(),
                    if change.forgot_okta {
                        " and the Okta credentials"
                    } else {
                        ""
                    }
                );
            }
            Ok(())
        }
        Err(KeydError::Absent) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// At startup, before the session is restored: a keyd installed after the
/// profile was saved, or one an update replaced, learns the site here.
pub fn tell_keyd() {
    let dir = crate::library::paths::data_dir();
    let Some(inst) = super::load(&dir) else {
        return;
    };
    if let Err(e) = tell(&dir, &inst) {
        elog!("[oculus] oculus-keyd did not take the site: {e}");
    }
}
