//! The Tauri commands Settings → Canvas calls.

use serde::Serialize;

use super::{apply, current, host_of, library_exists, normalise, probe, LOCKED};

#[derive(Serialize)]
pub struct InstitutionView {
    pub canvas: Option<String>,
    pub host: Option<String>,
    pub okta: bool,
    pub sso_host: Option<String>,
    /// The address cannot change while a library exists.
    pub locked: bool,
}

async fn view() -> Result<InstitutionView, String> {
    let inst = current();
    Ok(InstitutionView {
        host: inst.as_ref().and_then(|i| host_of(&i.canvas)),
        okta: inst.as_ref().is_some_and(|i| i.is_okta()),
        sso_host: inst.as_ref().and_then(|i| i.sso_host.clone()),
        canvas: inst.map(|i| i.canvas),
        locked: library_exists().await?,
    })
}

#[tauri::command]
pub async fn institution_get() -> Result<InstitutionView, String> {
    view().await
}

#[tauri::command]
pub async fn institution_configure(
    app: tauri::AppHandle,
    address: String,
) -> Result<InstitutionView, String> {
    let wanted = normalise(&address)?;
    let changing = current().is_none_or(|i| i.canvas != wanted);
    if changing && library_exists().await? {
        return Err(LOCKED.to_string());
    }
    let dir = crate::library::paths::data_dir();
    let probed = {
        let dir = dir.clone();
        crate::runtime::blocking::run(move || probe(&dir, &address)).await?
    };
    if changing {
        // Only once the new address checks out, and while the old hosts are
        // still the profile's, so the browser jar loses the old session.
        crate::auth::commands::end_session(&app).await?;
    }
    crate::runtime::blocking::run(move || apply(&dir, probed)).await?;
    view().await
}
