mod commands;
mod dto;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::startup_args,
            commands::capabilities,
            commands::scan_paths,
            commands::set_rules,
            commands::set_conflict_policy,
            commands::set_placeholder,
            commands::set_scan_options,
            commands::get_rows,
            commands::set_row_excluded,
            commands::exclude_rows,
            commands::clear_exclusions,
            commands::set_long_paths,
            commands::set_missing_token,
            commands::rescan,
            commands::apply,
            commands::list_history,
            commands::undo_batch,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
            commands::import_preset,
            commands::export_preset,
            commands::regex_test,
            commands::export_plan,
            commands::find_dupes,
            commands::detect_fs,
            commands::watch_start,
            commands::watch_stop,
            commands::watch_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ZRename");
}
