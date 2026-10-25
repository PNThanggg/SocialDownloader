mod window_fx;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(window_fx::WindowEffect::default())
        .invoke_handler(tauri::generate_handler![window_fx::window_effect])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window_fx::apply(&window);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
