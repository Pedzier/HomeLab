use crate::{
    commands::network::{choose, learned},
    models::Resolution,
    store::{main_only, validate_url, Store},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Rect};

// Must match the fixed React toolbar height in index.css (logical pixels).
const TOOLBAR_HEIGHT: f64 = 64.0;
const EMBEDDED_PREFIX: &str = "embedded-";
const MAX_CACHED_VIEWS: usize = 3;

#[derive(Default)]
pub struct ServiceViews {
    endpoints: Mutex<VecDeque<(String, String)>>,
    loaded: Mutex<HashMap<(String, String), Instant>>,
}

impl ServiceViews {
    pub fn was_loaded(&self, id: &str, url: &str) -> bool {
        let Ok(url) = validate_url(url) else {
            return false;
        };
        self.loaded.lock().ok().is_some_and(|entries| {
            entries
                .get(&(id.to_string(), url.origin().ascii_serialization()))
                .is_some_and(|at| at.elapsed() < Duration::from_secs(90))
        })
    }
}

#[derive(Clone, serde::Serialize)]
struct LoadedService {
    id: String,
    origin: String,
    status: String,
}

pub fn page_loaded(view: &tauri::Webview, payload: &tauri::webview::PageLoadPayload<'_>) {
    if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
        return;
    }
    let Some(id) = view
        .label()
        .strip_prefix(EMBEDDED_PREFIX)
        .or_else(|| view.label().strip_prefix("service-"))
    else {
        return;
    };
    if !matches!(payload.url().scheme(), "http" | "https") {
        return;
    }
    let origin = payload.url().origin().ascii_serialization();
    #[cfg(feature = "sandbox-check")]
    crate::sandbox_check::record_page(view.app_handle(), id, &origin);
    let Ok(config) = view.state::<Store>().get() else {
        return;
    };
    let Some(app) = config.apps.iter().find(|app| app.id == id) else {
        return;
    };
    let local = validate_url(&app.local_url)
        .ok()
        .is_some_and(|url| url.origin() == payload.url().origin());
    let remote = app
        .remote_url
        .as_ref()
        .and_then(|url| validate_url(url).ok())
        .is_some_and(|url| url.origin() == payload.url().origin());
    if !local && !remote {
        return;
    }
    let status = if local && app.connection_mode != "remote" {
        "LAN"
    } else {
        "Tailscale"
    }
    .to_string();

    let state = view.state::<ServiceViews>();
    if let Ok(mut entries) = state.loaded.lock() {
        entries.retain(|_, at| at.elapsed() < Duration::from_secs(90));
        entries.insert((id.into(), origin.clone()), Instant::now());
    }
    let _ = view.emit_to(
        "main",
        "service-loaded",
        LoadedService {
            id: id.into(),
            origin,
            status,
        },
    );
}

fn remember_view(
    entries: &mut VecDeque<(String, String)>,
    label: String,
    url: String,
) -> Option<String> {
    entries.retain(|(key, _)| key != &label);
    entries.push_back((label, url));
    if entries.len() > MAX_CACHED_VIEWS {
        entries.pop_front().map(|(label, _)| label)
    } else {
        None
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OpenTarget {
    Embedded,
    Window,
    External,
}

fn reusable_page(
    cached_endpoint: Option<&str>,
    current: Option<&tauri::Url>,
    target: &tauri::Url,
) -> bool {
    cached_endpoint == Some(target.as_str())
        && current.is_some_and(|current| {
            matches!(current.scheme(), "http" | "https") && current.origin() == target.origin()
        })
}

fn service_bounds(size: LogicalSize<f64>, top_inset: f64) -> Rect {
    Rect {
        position: LogicalPosition::new(0.0, top_inset + TOOLBAR_HEIGHT).into(),
        size: LogicalSize::new(
            size.width,
            (size.height - top_inset - TOOLBAR_HEIGHT).max(1.0),
        )
        .into(),
    }
}

async fn embedded_bounds(handle: &tauri::AppHandle) -> Result<Rect, String> {
    let window = handle.get_window("main").ok_or("Brak głównego okna")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let native_window = window.clone();
    window
        .run_on_main_thread(move || {
            let metrics =
                content_metrics(&native_window).map(|(size, inset)| service_bounds(size, inset));
            let _ = send.send(metrics);
        })
        .map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}

// WKWebView automatically insets the HTML viewport below the macOS titlebar.
// Native child views do not apply that same inset. Read AppKit's actual safe
// content area on the UI thread instead of subtracting Tauri outer/inner sizes.
#[cfg(target_os = "macos")]
fn content_metrics(window: &tauri::Window) -> Result<(LogicalSize<f64>, f64), String> {
    use objc2_app_kit::{NSView, NSWindow};
    let _main_thread = objc2::MainThreadMarker::new().ok_or("Układ wymaga głównego wątku")?;
    let view = unsafe {
        &*window
            .ns_view()
            .map_err(|e| e.to_string())?
            .cast::<NSView>()
    };
    let native = unsafe {
        &*window
            .ns_window()
            .map_err(|e| e.to_string())?
            .cast::<NSWindow>()
    };
    let frame = view.bounds();
    let safe = view.safeAreaInsets();
    let layout = native.contentLayoutRect();
    let inset = safe
        .top
        .max(frame.size.height - layout.size.height)
        .max(0.0);
    Ok((LogicalSize::new(frame.size.width, frame.size.height), inset))
}

#[cfg(not(target_os = "macos"))]
fn content_metrics(window: &tauri::Window) -> Result<(LogicalSize<f64>, f64), String> {
    Ok((
        window
            .inner_size()
            .map_err(|e| e.to_string())?
            .to_logical(window.scale_factor().map_err(|e| e.to_string())?),
        0.0,
    ))
}

pub async fn resize_embedded(handle: &tauri::AppHandle) -> Result<(), String> {
    let views: Vec<_> = handle
        .webviews()
        .into_values()
        .filter(|view| view.label().starts_with(EMBEDDED_PREFIX))
        .collect();
    if !views.is_empty() {
        let bounds = embedded_bounds(handle).await?;
        for view in views {
            view.set_bounds(bounds).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn close_service(window: tauri::Webview, handle: tauri::AppHandle) -> Result<(), String> {
    main_only(&window)?;
    for view in handle
        .webviews()
        .into_values()
        .filter(|view| view.label().starts_with(EMBEDDED_PREFIX))
    {
        view.hide().map_err(|e| e.to_string())?;
    }
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn reload_service(
    window: tauri::Webview,
    handle: tauri::AppHandle,
    state: tauri::State<'_, Store>,
    views: tauri::State<'_, ServiceViews>,
    id: String,
) -> Result<Resolution, String> {
    main_only(&window)?;
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let label = format!("{EMBEDDED_PREFIX}{uuid}");
    let previous = views
        .endpoints
        .lock()
        .map_err(|e| e.to_string())?
        .iter()
        .find(|(key, _)| key == &label)
        .map(|(_, url)| url.clone());
    let current = handle.get_webview(&label).and_then(|view| view.url().ok());
    // A reload must resolve the current LAN/remote route, not keep reloading an old VPN URL.
    *state.network_cache.lock().map_err(|e| e.to_string())? = None;
    let resolution = open_app(
        window,
        handle.clone(),
        state,
        views,
        id,
        OpenTarget::Embedded,
    )
    .await?;
    let target = validate_url(resolution.url.as_deref().ok_or("Brak adresu usługi")?)?;
    // open_app already navigates when the cached page is blank or at an old origin.
    if reusable_page(previous.as_deref(), current.as_ref(), &target) {
        handle
            .get_webview(&label)
            .ok_or("Widok usługi nie istnieje")?
            // A failed provisional HTTPS navigation can leave the view at about:blank.
            // Explicitly retry the configured endpoint instead of reloading that blank page.
            .navigate(target)
            .map_err(|e| e.to_string())?;
    }
    Ok(resolution)
}

async fn prepare_certificate_handler(
    view: &tauri::Webview,
    handle: &tauri::AppHandle,
    id: &str,
    name: &str,
    url: &tauri::Url,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if url.scheme() == "https" {
        if let Err(error) = super::certificates::install(view, handle, id, name, url).await {
            let _ = view.close();
            return Err(error);
        }
    }
    Ok(())
}

fn initial_url(url: &tauri::Url) -> tauri::Url {
    if url.scheme() == "https" {
        tauri::Url::parse("about:blank").expect("static URL")
    } else {
        url.clone()
    }
}

#[tauri::command]
pub async fn open_app(
    window: tauri::Webview,
    handle: tauri::AppHandle,
    state: tauri::State<'_, Store>,
    views: tauri::State<'_, ServiceViews>,
    id: String,
    target: OpenTarget,
) -> Result<Resolution, String> {
    main_only(&window)?;
    #[cfg(debug_assertions)]
    let started = std::time::Instant::now();
    let config = state.get()?;
    let app = config
        .apps
        .iter()
        .find(|a| a.id == id)
        .ok_or("Aplikacja nie istnieje")?;
    // Explicit modes already determine the URL and must not wait for network discovery.
    let trusted = if app.connection_mode == "auto" {
        learned(&state).await?.is_some_and(|n| n.trusted_local)
    } else {
        false
    };
    let resolution = choose(app, trusted);
    #[cfg(debug_assertions)]
    let resolved_at = std::time::Instant::now();
    let url = validate_url(resolution.url.as_deref().ok_or(resolution.reason.clone())?)?;
    let uuid = uuid::Uuid::parse_str(&app.id).map_err(|e| e.to_string())?;
    match target {
        OpenTarget::External => {
            let status = tokio::process::Command::new("/usr/bin/open")
                .arg(url.as_str())
                .status()
                .await
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("Nie udało się otworzyć przeglądarki".into());
            }
        }
        OpenTarget::Window => {
            let label = format!("service-{uuid}");
            if let Some(existing) = handle.get_webview_window(&label) {
                let view = handle.get_webview(&label).ok_or("Brak widoku usługi")?;
                prepare_certificate_handler(&view, &handle, &app.id, &app.name, &url).await?;
                existing.navigate(url).map_err(|e| e.to_string())?;
                existing.set_focus().map_err(|e| e.to_string())?;
            } else {
                tauri::WebviewWindowBuilder::new(
                    &handle,
                    &label,
                    tauri::WebviewUrl::External(initial_url(&url)),
                )
                .title(format!("{} — HomeLab", app.name))
                .inner_size(1180.0, 800.0)
                .data_store_identifier(*uuid.as_bytes())
                .on_navigation(|u| {
                    matches!(u.scheme(), "http" | "https") || u.as_str() == "about:blank"
                })
                .build()
                .map_err(|e| e.to_string())?;
                let view = handle.get_webview(&label).ok_or("Brak widoku usługi")?;
                prepare_certificate_handler(&view, &handle, &app.id, &app.name, &url).await?;
                if url.scheme() == "https" {
                    view.navigate(url).map_err(|e| e.to_string())?;
                }
            }
        }
        OpenTarget::Embedded => {
            let label = format!("{EMBEDDED_PREFIX}{uuid}");
            let bounds = embedded_bounds(&handle).await?;
            let endpoint = url.as_str().to_string();
            if let Some(existing) = handle.get_webview(&label) {
                prepare_certificate_handler(&existing, &handle, &app.id, &app.name, &url).await?;
                let cached_endpoint = views
                    .endpoints
                    .lock()
                    .map_err(|e| e.to_string())?
                    .iter()
                    .find(|(key, _)| key == &label)
                    .map(|(_, value)| value.clone());
                let current = existing.url().ok();
                // Reuse only an actual page at the selected origin. A remembered
                // request URL alone does not prove that the navigation succeeded.
                if !reusable_page(cached_endpoint.as_deref(), current.as_ref(), &url) {
                    existing.navigate(url).map_err(|e| e.to_string())?;
                }
                existing.set_bounds(bounds).map_err(|e| e.to_string())?;
                existing.show().map_err(|e| e.to_string())?;
                existing.set_focus().map_err(|e| e.to_string())?;
            } else {
                let parent = handle.get_window("main").ok_or("Brak głównego okna")?;
                parent
                    .add_child(
                        tauri::webview::WebviewBuilder::new(
                            &label,
                            tauri::WebviewUrl::External(initial_url(&url)),
                        )
                        .data_store_identifier(*uuid.as_bytes())
                        .on_navigation(|u| {
                            matches!(u.scheme(), "http" | "https") || u.as_str() == "about:blank"
                        }),
                        bounds.position,
                        bounds.size,
                    )
                    .map_err(|e| e.to_string())?;
                let view = handle.get_webview(&label).ok_or("Brak widoku usługi")?;
                prepare_certificate_handler(&view, &handle, &app.id, &app.name, &url).await?;
                if url.scheme() == "https" {
                    view.navigate(url).map_err(|e| e.to_string())?;
                }
            }
            for view in handle
                .webviews()
                .into_values()
                .filter(|view| view.label().starts_with(EMBEDDED_PREFIX) && view.label() != label)
            {
                view.hide().map_err(|e| e.to_string())?;
            }
            let evicted = remember_view(
                &mut *views.endpoints.lock().map_err(|e| e.to_string())?,
                label,
                endpoint,
            );
            if let Some(evicted) = evicted {
                if let Some(view) = handle.get_webview(&evicted) {
                    view.close().map_err(|e| e.to_string())?;
                }
            }
        }
    }
    #[cfg(debug_assertions)]
    eprintln!(
        "HomeLab open: resolver={}ms, view={}ms",
        resolved_at.duration_since(started).as_millis(),
        resolved_at.elapsed().as_millis()
    );
    Ok(resolution)
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn cached_views_keep_recent_services_and_bound_memory() {
        let mut entries = VecDeque::new();
        for label in ["a", "b", "c"] {
            assert!(remember_view(&mut entries, label.into(), format!("http://{label}")).is_none());
        }
        assert!(remember_view(&mut entries, "a".into(), "http://a".into()).is_none());
        assert_eq!(entries.len(), 3);
        assert_eq!(
            remember_view(&mut entries, "d".into(), "http://d".into()),
            Some("b".into())
        );
        assert!(entries.iter().any(|(key, _)| key == "a"));
        remember_view(&mut entries, "a".into(), "https://new-route".into());
        assert_eq!(entries.back().unwrap().1, "https://new-route");
    }

    #[test]
    fn page_confirmation_expires_and_does_not_cross_lan_remote_or_apps() {
        let views = ServiceViews::default();
        views.loaded.lock().unwrap().insert(
            ("app".into(), "https://192.168.0.20:8006".into()),
            Instant::now(),
        );
        assert!(views.was_loaded("app", "https://192.168.0.20:8006/#login"));
        assert!(!views.was_loaded("other", "https://192.168.0.20:8006/"));
        assert!(!views.was_loaded("app", "https://100.107.29.27:8006/"));
        views.loaded.lock().unwrap().insert(
            ("app".into(), "https://192.168.0.20:8006".into()),
            Instant::now() - Duration::from_secs(91),
        );
        assert!(!views.was_loaded("app", "https://192.168.0.20:8006/"));
    }

    #[test]
    fn failed_or_old_origin_pages_are_navigated_again() {
        let target = tauri::Url::parse("https://192.168.0.20:8006/").unwrap();
        let blank = tauri::Url::parse("about:blank").unwrap();
        let old_http = tauri::Url::parse("http://192.168.0.20:8006/").unwrap();
        let remote = tauri::Url::parse("https://100.107.29.27:8006/").unwrap();
        let login = tauri::Url::parse("https://192.168.0.20:8006/#login").unwrap();
        let cached = Some(target.as_str());
        assert!(!reusable_page(cached, None, &target));
        for previous in [&blank, &old_http, &remote] {
            assert!(!reusable_page(cached, Some(previous), &target));
        }
        assert!(reusable_page(cached, Some(&login), &target));
        assert!(!reusable_page(Some(remote.as_str()), Some(&login), &target));
    }

    #[test]
    fn service_keeps_toolbar_clear_at_each_window_size_and_scale() {
        for scale in [1.0, 2.0] {
            for (width, height) in [(820.0, 600.0), (1180.0, 800.0), (1728.0, 1117.0)] {
                let physical = LogicalSize::new(width, height).to_physical::<u32>(scale);
                let inset = 32.0;
                let bounds = service_bounds(physical.to_logical::<f64>(scale), inset);
                let position = bounds.position.to_logical::<f64>(scale);
                let size = bounds.size.to_logical::<f64>(scale);
                assert_eq!(position.y, inset + TOOLBAR_HEIGHT);
                assert_eq!(size.width, width);
                assert_eq!(position.y + size.height, height);
            }
        }
    }
}
