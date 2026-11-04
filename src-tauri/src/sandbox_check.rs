//! Diagnostic build only. Never reads production credentials or accepts certificates.
use crate::{commands::network, models::HomelabApp, store::Store};
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

static REPORT: OnceLock<Mutex<Value>> = OnceLock::new();

fn record(app: &tauri::AppHandle, key: &str, value: Value) {
    let report = REPORT.get_or_init(|| Mutex::new(json!({})));
    if let Ok(mut report) = report.lock() {
        report[key] = value;
        if let Some(dir) = app.state::<Store>().path.parent() {
            if let Ok(bytes) = serde_json::to_vec_pretty(&*report) {
                let temporary = dir.join("sandbox-report.tmp");
                if std::fs::write(&temporary, bytes).is_ok() {
                    let _ = std::fs::rename(temporary, dir.join("sandbox-report.json"));
                }
            }
        }
    }
}

pub fn record_page(app: &tauri::AppHandle, id: &str, origin: &str) {
    record(
        app,
        &format!("webview:{id}"),
        json!({"origin": origin, "loaded": true}),
    );
}

pub fn start(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Refuse to run diagnostics in the normal app, even if the feature was enabled accidentally.
    if app.config().identifier != "pl.homelab.desktop.sandboxcheck" {
        return Err("sandbox-check requires the isolated sandbox configuration".into());
    }
    let fixture = app.path().resource_dir()?.join("sandbox-endpoints.json");
    let apps: Vec<HomelabApp> = serde_json::from_slice(&std::fs::read(fixture)?)?;
    app.state::<Store>().update(|config| {
        config.apps = apps;
        config.credentials.clear();
        Ok(())
    })?;
    record(
        app,
        "started",
        json!({
            "at": chrono::Utc::now().to_rfc3339(),
            "identifier": app.config().identifier,
            "config_path": app.state::<Store>().path,
            "signing": "local ad-hoc diagnostic build; not an App Store submission"
        }),
    );
    let app = app.clone();
    tauri::async_runtime::spawn(async move { run(&app).await });
    Ok(())
}

async fn command(program: &str, args: &[&str]) -> Value {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::process::Command::new(program)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    match result {
        Ok(Ok(output)) => json!({
            "success": output.status.success(), "code": output.status.code(),
            "stdout": String::from_utf8_lossy(&output.stdout).chars().take(6000).collect::<String>(),
            "stderr": String::from_utf8_lossy(&output.stderr).chars().take(1500).collect::<String>()
        }),
        Ok(Err(error)) => json!({"success": false, "error": error.to_string()}),
        Err(_) => json!({"success": false, "error": "timeout: 3 seconds"}),
    }
}

async fn run(app: &tauri::AppHandle) {
    let state = app.state::<Store>();
    // Store uses the real atomic-write implementation inside this app's container.
    let persistence = state.update(|_| Ok(())).and_then(|_| {
        let bytes = std::fs::read(&state.path).map_err(|e| e.to_string())?;
        serde_json::from_slice::<crate::models::Config>(&bytes).map_err(|e| e.to_string())?;
        Ok(())
    });
    record(
        app,
        "container_storage",
        json!({"ok": persistence.is_ok(), "error": persistence.err()}),
    );

    let outside = std::path::PathBuf::from(format!(
        "/private/tmp/homelab-sandbox-outside-{}",
        uuid::Uuid::new_v4()
    ));
    let attempt = std::fs::write(&outside, b"disposable sandbox boundary probe");
    let denied = attempt
        .as_ref()
        .is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
    if attempt.is_ok() {
        let _ = std::fs::remove_file(&outside);
    }
    record(
        app,
        "outside_container_write",
        json!({"blocked": denied, "error": attempt.err().map(|e| e.to_string())}),
    );

    let keychain = tauri::async_runtime::spawn_blocking(|| -> Result<(), String> {
        let entry = keyring::Entry::new(
            "pl.homelab.desktop.sandboxcheck",
            &format!("audit-{}", uuid::Uuid::new_v4()),
        )
        .map_err(|e| e.to_string())?;
        let result = (|| {
            entry
                .set_password("disposable-sandbox-test")
                .map_err(|e| e.to_string())?;
            if entry.get_password().map_err(|e| e.to_string())? != "disposable-sandbox-test" {
                return Err("Keychain returned a different test value".into());
            }
            entry
                .set_password("updated-sandbox-test")
                .map_err(|e| e.to_string())?;
            if entry.get_password().map_err(|e| e.to_string())? != "updated-sandbox-test" {
                return Err("Keychain update was not persisted".into());
            }
            Ok(())
        })();
        let cleanup = entry.delete_credential().map_err(|e| e.to_string());
        result.and(cleanup)
    })
    .await;
    let keychain = keychain.unwrap_or_else(|e| Err(e.to_string()));
    record(
        app,
        "keychain_disposable_entry",
        json!({"ok": keychain.is_ok(), "error": keychain.err()}),
    );

    for (key, program, args) in [
        ("route", "/sbin/route", vec!["-n", "get", "default"]),
        ("interfaces", "/sbin/ifconfig", vec!["-a"]),
        ("arp", "/usr/sbin/arp", vec!["-an"]),
        (
            "hardware_ports",
            "/usr/sbin/networksetup",
            vec!["-listallhardwareports"],
        ),
    ] {
        record(app, key, command(program, &args).await);
    }

    // Exercise the actual resolver and its normal endpoint probes, not a separate test implementation.
    let network = network::learned(&state).await;
    match network {
        Ok(profile) => record(app, "resolver", json!({"ok": true, "network": profile})),
        Err(error) => record(app, "resolver", json!({"ok": false, "error": error})),
    }
    record(app, "completed", json!(chrono::Utc::now().to_rfc3339()));
}
