#![deny(clippy::all)]

mod audio;
use audio::AudioState;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .level_for(
                    "tcp_streamer_lib",
                    if cfg!(debug_assertions) {
                        log::LevelFilter::Debug
                    } else {
                        log::LevelFilter::Info
                    },
                )
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .manage(AudioState::new())
        .setup(|app| {
            // Check if launched with autostart (minimized flag)
            let args: Vec<String> = std::env::args().collect();
            let start_hidden = args
                .iter()
                .any(|arg| arg == "--hidden" || arg == "--minimized" || arg.contains("autostart"));

            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show TCP Streamer", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            // tray-icon's Linux default uses process-local counters in a shared directory.
            // Other apps can overwrite/remove the same tray-icon-<id>-0.png. Namespace
            // the files by our application identifier, even when launched hidden.
            let tray = TrayIconBuilder::with_id("com.jdavidoa91.tcp-streamer.tray");
            #[cfg(target_os = "linux")]
            let tray = tray.temp_dir_path(app.path().app_cache_dir()?.join("tray"));

            let _tray = tray
                .menu(&menu)
                .tooltip("TCP Streamer")
                .show_menu_on_left_click(false)
                .icon(
                    app.default_window_icon()
                        .ok_or("Missing application icon")?
                        .clone(),
                )
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        show_main_window(app);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        show_main_window(app);
                    }
                })
                .build(app)?;

            // If started hidden (e.g., from autostart), hide the window
            if start_hidden {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            audio::get_input_devices,
            audio::get_output_devices,
            audio::list_sources,
            audio::start_stream,
            audio::start_sink,
            audio::stop_stream,
            audio::get_stream_state,
            audio::diagnostics::preflight_stream,
            audio::get_os_type,
            audio::get_local_ip
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AudioState>();
                state.shutdown();
            }
        });
}
