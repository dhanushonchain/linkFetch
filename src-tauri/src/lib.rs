use tauri::Manager;

#[cfg(test)]
mod live_tests;

mod commands;
mod download;
mod error;
mod models;
mod security;
mod sources;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(commands::AppState::new());
            // Keep the window compact no matter what the platform prefers.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_size(tauri::LogicalSize::new(460.0, 380.0));
                let _ = window.set_min_size(Some(tauri::LogicalSize::new(420.0, 320.0)));
                let _ = window.center();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_download,
            commands::cancel_download,
            commands::dependency_status,
            commands::default_destination_command,
            commands::pick_destination,
            commands::reveal_in_finder,
            commands::open_file,
            commands::describe_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running linkFetch");
}
