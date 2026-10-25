mod accounts;
mod download;
mod errors;
mod model;
mod resolve;
mod site;
mod thumb;
mod tools;
mod window_fx;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(window_fx::WindowEffect::default())
        .manage(tools::ToolsState::default())
        .manage(resolve::ResolveCache::default())
        .manage(download::jobs::JobManager::default())
        .invoke_handler(tauri::generate_handler![
            accounts::accounts_status,
            accounts::connect_account,
            accounts::disconnect_account,
            window_fx::window_effect,
            tools::tools_status,
            tools::tool_download_size,
            tools::install_tool,
            tools::update_tools,
            download::filename::default_filename,
            download::jobs::download,
            download::jobs::download_all,
            download::jobs::cancel_job,
            resolve::resolve,
            thumb::fetch_thumbnail
        ])
        .setup(|app| {
            // Before any job can start: drop job dirs and decrypted cookie files a crash left behind.
            tools::paths::sweep_tmp();
            if let Some(window) = app.get_webview_window("main") {
                window_fx::apply(&window);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
