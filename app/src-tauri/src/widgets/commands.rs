/// TSX source of one widget -> ES module JS (`transpile.rs`).
#[tauri::command]
pub async fn widget_transpile(source: String) -> Result<super::transpile::Transpiled, String> {
    crate::runtime::blocking::run(move || super::transpile::transpile(&source)).await
}
