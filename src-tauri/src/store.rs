use crate::models::*;
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;
pub struct Store {
    pub path: PathBuf,
    pub config: Mutex<Config>,
    pub network_cache: Mutex<Option<crate::commands::network::LearnedNetworkCache>>,
    pub resolver_lock: tokio::sync::Mutex<()>,
}
pub fn main_only(window: &tauri::Webview) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Access denied".into());
    }
    Ok(())
}
impl Store {
    pub fn load(app: &tauri::AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("settings.json");
        let config = if path.exists() {
            serde_json::from_slice(&std::fs::read(&path)?)?
        } else {
            seed()
        };
        let store = Self {
            path,
            config: Mutex::new(config),
            network_cache: Mutex::new(None),
            resolver_lock: tokio::sync::Mutex::new(()),
        };
        store.update(|_| Ok(()))?;
        Ok(store)
    }
    pub fn get(&self) -> Result<Config, String> {
        self.config
            .lock()
            .map(|c| c.clone())
            .map_err(|e| e.to_string())
    }
    pub fn update(&self, f: impl FnOnce(&mut Config) -> Result<(), String>) -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut guard = self.config.lock().map_err(|e| e.to_string())?;
        let mut next = guard.clone();
        f(&mut next)?;
        let tmp = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(tmp, &self.path).map_err(|e| e.to_string())?;
        *guard = next;
        Ok(())
    }
}
pub fn validate_url(value: &str) -> Result<tauri::Url, String> {
    let url = tauri::Url::parse(value)
        .map_err(|_| "Podaj pełny adres http:// lub https://".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Adres musi używać HTTP(S) i nie może zawierać hasła ani loginu".into());
    }
    Ok(url)
}
#[tauri::command]
pub fn load_config(window: tauri::Webview, state: tauri::State<Store>) -> Result<Config, String> {
    main_only(&window)?;
    state.get()
}
#[tauri::command]
pub fn save_app(
    window: tauri::Webview,
    state: tauri::State<Store>,
    app: HomelabApp,
) -> Result<(), String> {
    main_only(&window)?;
    validate_url(&app.local_url)?;
    if let Some(url) = &app.remote_url {
        validate_url(url)?;
    }
    if app.name.trim().is_empty()
        || !["auto", "local", "remote"].contains(&app.connection_mode.as_str())
    {
        return Err("Nieprawidłowa nazwa lub tryb połączenia".into());
    }
    uuid::Uuid::parse_str(&app.id).map_err(|e| e.to_string())?;
    state.update(|c| {
        if let Some(id) = &app.credential_id {
            if !c.credentials.iter().any(|x| &x.id == id) {
                return Err("Credential nie istnieje".into());
            }
        }
        if let Some(old) = c.apps.iter_mut().find(|a| a.id == app.id) {
            *old = app;
        } else {
            c.apps.push(app);
        }
        Ok(())
    })
}
#[tauri::command]
pub fn delete_app(
    window: tauri::Webview,
    state: tauri::State<Store>,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    state.update(|c| {
        c.apps.retain(|a| a.id != id);
        c.trusted_certificates.retain(|p| p.app_id != id);
        Ok(())
    })
}
#[tauri::command]
pub fn reorder_apps(
    window: tauri::Webview,
    state: tauri::State<Store>,
    ids: Vec<String>,
) -> Result<(), String> {
    main_only(&window)?;
    state.update(|c| {
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        if ids.len() != c.apps.len()
            || unique.len() != ids.len()
            || c.apps.iter().any(|a| !ids.contains(&a.id))
        {
            return Err("Nieprawidłowa kolejność".into());
        }
        for app in &mut c.apps {
            app.order = ids.iter().position(|id| id == &app.id).unwrap();
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_settings_need_no_migration_for_certificate_exceptions() {
        let config: Config =
            serde_json::from_str(r#"{"apps":[],"credentials":[],"networks":[]}"#).unwrap();
        assert!(config.trusted_certificates.is_empty());
    }

    #[test]
    fn storage_is_atomic_and_survives_reload() {
        let dir = std::env::temp_dir().join(format!("homelab-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store {
            path: dir.join("settings.json"),
            config: Mutex::new(seed()),
            network_cache: Mutex::new(None),
            resolver_lock: tokio::sync::Mutex::new(()),
        };
        store
            .update(|c| {
                c.apps[0].name = "Persisted".into();
                Ok(())
            })
            .unwrap();
        let loaded: Config = serde_json::from_slice(&std::fs::read(&store.path).unwrap()).unwrap();
        assert_eq!(loaded.apps[0].name, "Persisted");
        assert!(store
            .update(|c| {
                c.apps.clear();
                Err("rollback".into())
            })
            .is_err());
        assert_eq!(store.get().unwrap().apps.len(), 4);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_credentials_and_executable_schemes_in_urls() {
        for bad in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "http://admin:secret@host",
            "ftp://host",
            "http://user@host",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
        assert!(validate_url("http://192.168.0.20:8123/path").is_ok());
        assert!(validate_url("https://example.ts.net").is_ok());
    }
}
