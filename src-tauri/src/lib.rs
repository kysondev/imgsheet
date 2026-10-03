mod commands;
mod error;
mod imaging;
mod pdf;
#[cfg(test)]
mod tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::import_photo, commands::export_pdf])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
