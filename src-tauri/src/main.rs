#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod commands;
mod models;
#[cfg(feature = "sandbox-check")]
mod sandbox_check;
mod store;
use tauri::Manager;
fn main() {
    tauri::Builder::default()
        .on_page_load(commands::webview::page_loaded)
        .setup(|app| {
            app.manage(store::Store::load(app.handle())?);
            app.manage(commands::webview::ServiceViews::default());
            #[cfg(feature = "sandbox-check")]
            sandbox_check::start(app.handle())?;
            // Keep the trusted dashboard and service webviews as sibling views
            // so their native layout and permissions can be controlled separately.
            let window =
                tauri::window::WindowBuilder::from_config(app, &app.config().app.windows[0])?
                    .build()?;
            window.add_child(
                tauri::webview::WebviewBuilder::new(
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .auto_resize(),
                tauri::LogicalPosition::new(0.0, 0.0),
                window.inner_size()?,
            )?;
            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
                ) {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = commands::webview::resize_embedded(&handle).await;
                    });
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            store::load_config,
            store::save_app,
            store::delete_app,
            store::reorder_apps,
            commands::network::current_network,
            commands::network::resolve_app,
            commands::network::check_services,
            commands::network::trust_network,
            commands::network::forget_network,
            commands::webview::open_app,
            commands::webview::close_service,
            commands::webview::reload_service,
            commands::certificates::forget_certificates,
            commands::credentials::save_credential,
            commands::credentials::delete_credential,
            commands::credentials::copy_credential
        ])
        .run(tauri::generate_context!())
        .expect("Nie udało się uruchomić HomeLab");
}
