use crate::{
    models::Credential,
    store::{main_only, Store},
};
use std::io::Write;
const SERVICE: &str = "pl.homelab.desktop";
fn entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn save_credential(
    window: tauri::Webview,
    state: tauri::State<Store>,
    id: String,
    name: String,
    username: String,
    notes: String,
    password: Option<String>,
) -> Result<(), String> {
    main_only(&window)?;
    uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    if name.trim().is_empty() {
        return Err("Podaj nazwę credentiala".into());
    }
    let old = state.get()?.credentials.iter().any(|c| c.id == id);
    if !old && password.as_deref().is_none_or(str::is_empty) {
        return Err("Podaj hasło".into());
    }
    if let Some(secret) = password {
        entry(&id)?
            .set_password(&secret)
            .map_err(|e| format!("Keychain: {e}"))?;
    }
    state.update(|c| {
        let credential = Credential {
            id: id.clone(),
            name,
            username: Some(username),
            notes: Some(notes),
            secret_reference: format!("keychain:{SERVICE}:{id}"),
        };
        if let Some(old) = c.credentials.iter_mut().find(|x| x.id == id) {
            *old = credential;
        } else {
            c.credentials.push(credential);
        }
        Ok(())
    })
}
#[tauri::command]
pub fn delete_credential(
    window: tauri::Webview,
    state: tauri::State<Store>,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    if !state.get()?.credentials.iter().any(|c| c.id == id) {
        return Err("Credential nie istnieje".into());
    }
    match entry(&id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(format!("Keychain: {e}")),
    }
    state.update(|c| {
        c.credentials.retain(|x| x.id != id);
        for app in &mut c.apps {
            if app.credential_id.as_deref() == Some(&id) {
                app.credential_id = None;
            }
        }
        Ok(())
    })
}
#[tauri::command]
pub fn copy_credential(
    window: tauri::Webview,
    state: tauri::State<Store>,
    id: String,
    field: String,
) -> Result<(), String> {
    main_only(&window)?;
    let config = state.get()?;
    let c = config
        .credentials
        .iter()
        .find(|c| c.id == id)
        .ok_or("Credential nie istnieje")?;
    let text = match field.as_str() {
        "password" => entry(&id)?
            .get_password()
            .map_err(|e| format!("Keychain: {e}"))?,
        "username" => c.username.clone().unwrap_or_default(),
        _ => return Err("Nieprawidłowe pole".into()),
    };
    let mut child = std::process::Command::new("/usr/bin/pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("Brak schowka")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    if !child.wait().map_err(|e| e.to_string())?.success() {
        return Err("Nie udało się skopiować".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Accesses macOS Keychain using a disposable test entry"]
    fn keychain_roundtrip() {
        let id = format!("smoke-test-{}", uuid::Uuid::new_v4());
        let e = super::entry(&id).unwrap();
        e.set_password("homelab-disposable-test-value").unwrap();
        assert_eq!(e.get_password().unwrap(), "homelab-disposable-test-value");
        e.set_password("homelab-updated-test-value").unwrap();
        assert_eq!(e.get_password().unwrap(), "homelab-updated-test-value");
        e.delete_credential().unwrap();
        assert!(matches!(e.get_password(), Err(keyring::Error::NoEntry)));
    }
}
