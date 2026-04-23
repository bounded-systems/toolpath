#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod error;
mod tray;

use commands::{derive, export, keychain, sources, upload};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            tray::install(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sources::list_agents,
            sources::list_claude_projects,
            sources::list_claude_projects_stream,
            sources::list_claude_sessions,
            sources::list_claude_sessions_stream,
            sources::claude_session_title,
            sources::list_pi_projects_stream,
            sources::list_pi_sessions_stream,
            sources::list_git_branches,
            derive::derive_claude,
            derive::derive_pi,
            derive::derive_git,
            derive::derive_github,
            export::save_document,
            upload::upload_to_pathbase,
            keychain::github_set_token,
            keychain::github_has_token,
            keychain::github_clear_token,
            tray::tray_stats_now,
            tray::tray_open_main,
            tray::tray_open_trace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running toolpath-desktop");
}
