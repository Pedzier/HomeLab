//! macOS-only certificate exceptions. The original Wry navigation delegate is
//! owned by Wry and receives all callbacks other than server trust authentication.
use crate::{
    models::TrustedCertificate,
    store::{main_only, Store},
};
use block2::{DynBlock, RcBlock};
use core_foundation::base::TCFType;
use objc2::{
    define_class, msg_send,
    rc::{Retained, Weak},
    runtime::{AnyObject, NSObject, ProtocolObject, Sel},
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{NSAlert, NSAlertStyle, NSModalResponse};
use objc2_foundation::{
    NSObjectProtocol, NSString, NSURLAuthenticationChallenge, NSURLCredential,
    NSURLSessionAuthChallengeDisposition as Disposition,
};
use objc2_web_kit::{WKNavigationDelegate, WKWebView};
use security_framework::trust::SecTrust;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::c_void,
    ptr,
};
use tauri::{Emitter, Manager};
use x509_parser::prelude::{FromDer, X509Certificate};

const MAX_PIN_DAYS: i64 = 30;
static DELEGATE_KEY: u8 = 0;
unsafe extern "C" {
    fn objc_setAssociatedObject(
        object: *mut AnyObject,
        key: *const c_void,
        value: *mut AnyObject,
        policy: usize,
    );
    fn objc_getAssociatedObject(object: *mut AnyObject, key: *const c_void) -> *mut AnyObject;
}

#[derive(Clone)]
struct Scope {
    view_label: String,
    app_id: String,
    name: String,
    host: String,
    port: u16,
}
impl Scope {
    fn matches(&self, host: &str, port: u16) -> bool {
        self.host == host && self.port == port
    }
}
fn can_offer_exception(status: i32, result: objc2_security::SecTrustResultType) -> bool {
    status == 0 && result == objc2_security::SecTrustResultType::RecoverableTrustFailure
}

fn pin_matches(pin: &TrustedCertificate, scope: &Scope, fingerprint: &str, now: i64) -> bool {
    pin.app_id == scope.app_id
        && pin.host == scope.host
        && pin.port == scope.port
        && pin.sha256 == fingerprint
        && pin.expires_at > now
}

pub struct DelegateIvars {
    original: Weak<ProtocolObject<dyn WKNavigationDelegate>>,
    handle: tauri::AppHandle,
    scope: RefCell<Scope>,
    session: RefCell<HashMap<(String, u16, String), i64>>,
    prompting: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    struct CertificateDelegate;

    unsafe impl NSObjectProtocol for CertificateDelegate {
        #[unsafe(method(respondsToSelector:))]
        fn responds_to_selector(&self, selector: Sel) -> bool {
            let own: bool = unsafe { msg_send![super(self), respondsToSelector: selector] };
            own || self.ivars().original.load().is_some_and(|original| original.respondsToSelector(selector))
        }
    }
    impl CertificateDelegate {
        #[unsafe(method(forwardingTargetForSelector:))]
        fn forwarding_target(&self, _selector: Sel) -> *mut AnyObject {
            self.ivars().original.load().map(|original| Retained::autorelease_ptr(original).cast()).unwrap_or(ptr::null_mut())
        }
    }
    unsafe impl WKNavigationDelegate for CertificateDelegate {
        #[unsafe(method(webView:didReceiveAuthenticationChallenge:completionHandler:))]
        fn authentication_challenge(&self, webview: &WKWebView, challenge: &NSURLAuthenticationChallenge,
            completion: &DynBlock<dyn Fn(Disposition, *mut NSURLCredential)>) {
            self.handle_challenge(webview, challenge, completion);
        }
    }
);

fn accept(completion: &DynBlock<dyn Fn(Disposition, *mut NSURLCredential)>, trust: &SecTrust) {
    let credential: Retained<NSURLCredential> = unsafe {
        msg_send![NSURLCredential::class(), credentialForTrust: trust.as_concrete_TypeRef().cast::<objc2_security::SecTrust>()]
    };
    completion.call((
        Disposition::UseCredential,
        Retained::as_ptr(&credential).cast_mut(),
    ));
}

impl CertificateDelegate {
    fn handle_challenge(
        &self,
        webview: &WKWebView,
        challenge: &NSURLAuthenticationChallenge,
        completion: &DynBlock<dyn Fn(Disposition, *mut NSURLCredential)>,
    ) {
        let space = challenge.protectionSpace();
        if space.authenticationMethod().to_string() != "NSURLAuthenticationMethodServerTrust" {
            completion.call((Disposition::PerformDefaultHandling, ptr::null_mut()));
            return;
        }
        let pointer: *mut c_void = unsafe { msg_send![&*space, serverTrust] };
        if pointer.is_null() {
            completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
            return;
        }
        // serverTrust is a borrowed Security.framework SecTrustRef. Retain it
        // until the asynchronous sheet's completion handler has been called.
        let trust = unsafe { SecTrust::wrap_under_get_rule(pointer.cast()) };
        let validation = trust.evaluate_with_error();
        if validation.is_ok() {
            completion.call((Disposition::PerformDefaultHandling, ptr::null_mut()));
            return;
        }
        // Respect explicit system denials and fatal failures (e.g. revocation).
        // Only failures which Security.framework marks as user-overridable
        // may reach our prompt or a previously saved exception.
        let mut result = objc2_security::SecTrustResultType::Invalid;
        let status = unsafe {
            let native_trust = &*trust
                .as_concrete_TypeRef()
                .cast::<objc2_security::SecTrust>();
            native_trust.trust_result(std::ptr::NonNull::from(&mut result))
        };
        if !can_offer_exception(status, result) {
            completion.call((Disposition::PerformDefaultHandling, ptr::null_mut()));
            return;
        }
        let scope = self.ivars().scope.borrow().clone();
        let host = space.host().to_string().to_lowercase();
        let Ok(port) = u16::try_from(space.port()) else {
            completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
            return;
        };
        // A redirect or subresource cannot silently gain a certificate exception.
        if !scope.matches(&host, port) {
            completion.call((Disposition::PerformDefaultHandling, ptr::null_mut()));
            return;
        }
        let Some(certificate) = trust.chain().into_iter().next() else {
            completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
            return;
        };
        let der = certificate.to_der();
        let fingerprint = Sha256::digest(&der)
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":");
        let session_key = (host.clone(), port, fingerprint.clone());
        let now = chrono::Utc::now().timestamp();
        let pinned = self
            .ivars()
            .handle
            .state::<Store>()
            .get()
            .ok()
            .is_some_and(|c| {
                c.trusted_certificates
                    .iter()
                    .any(|p| pin_matches(p, &scope, &fingerprint, now))
            });
        if pinned
            || self
                .ivars()
                .session
                .borrow()
                .get(&session_key)
                .is_some_and(|expires| *expires > now)
        {
            accept(completion, &trust);
            return;
        }
        if self.ivars().prompting.replace(true) {
            completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
            return;
        }
        let Some(parent) = webview.window() else {
            self.ivars().prompting.set(false);
            completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
            return;
        };
        let parsed = X509Certificate::from_der(&der).ok().map(|(_, cert)| cert);
        let validity = parsed
            .as_ref()
            .map(|cert| {
                format!(
                    "{} – {}",
                    cert.validity().not_before,
                    cert.validity().not_after
                )
            })
            .unwrap_or_else(|| "Nie można odczytać".into());
        let valid_until = parsed
            .as_ref()
            .filter(|cert| cert.validity().is_valid())
            .map(|cert| cert.validity().not_after.timestamp());
        let issuer = parsed
            .as_ref()
            .map(|cert| cert.issuer().to_string())
            .unwrap_or_else(|| "Nieznany".into());
        let alert = NSAlert::new(self.mtm());
        alert.setAlertStyle(NSAlertStyle::Warning);
        alert.setMessageText(&NSString::from_str(&format!(
            "Czy zaufać certyfikatowi {}?",
            scope.name
        )));
        let error = validation.err().map(|e| e.to_string()).unwrap_or_default();
        alert.setInformativeText(&NSString::from_str(&format!(
            "Serwer: {host}:{port}\n\nmacOS nie może potwierdzić tożsamości tego serwera. Certyfikat może być prywatny, wygasły lub wystawiony dla innego adresu.\n\nPowód: {error}\nWłaściciel: {}\nWystawca: {issuer}\nWażność: {validity}\n\nOdcisk SHA-256:\n{fingerprint}\n\nPołącz się tylko, jeśli rozpoznajesz ten serwer. Zapamiętanie dotyczy wyłącznie tego adresu, portu i certyfikatu w HomeLab, maksymalnie przez {MAX_PIN_DAYS} dni. Zmiana certyfikatu wymaga ponownej zgody.", certificate.subject_summary()
        )));
        // Cancel is the default action. Enter must not silently grant trust.
        alert.addButtonWithTitle(&NSString::from_str("Anuluj"));
        alert.addButtonWithTitle(&NSString::from_str("Połącz tym razem"));
        if valid_until.is_some() {
            alert.addButtonWithTitle(&NSString::from_str("Zaufaj i zapamiętaj"));
        }
        let completion = completion.copy();
        let delegate = unsafe { Retained::retain(self as *const Self as *mut Self) }
            .expect("delegate is non-null");
        let callback = RcBlock::new(move |response: NSModalResponse| {
            delegate.ivars().prompting.set(false);
            if response != 1001 && response != 1002 {
                completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
                if scope.view_label.starts_with("embedded-") {
                    if let Some(view) = delegate.ivars().handle.get_webview(&scope.view_label) {
                        let _ = view.close();
                    }
                    let _ = delegate.ivars().handle.emit_to(
                        "main",
                        "certificate-cancelled",
                        scope.app_id.clone(),
                    );
                }
                return;
            }
            if response == 1002 {
                let Some(valid_until) = valid_until else {
                    completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
                    return;
                };
                let expires_at = valid_until.min(now + MAX_PIN_DAYS * 86400);
                let result = delegate.ivars().handle.state::<Store>().update(|c| {
                    c.trusted_certificates.retain(|p| {
                        !(p.app_id == scope.app_id && p.host == host && p.port == port)
                    });
                    c.trusted_certificates.push(TrustedCertificate {
                        app_id: scope.app_id.clone(),
                        host: host.clone(),
                        port,
                        sha256: fingerprint.clone(),
                        expires_at,
                    });
                    Ok(())
                });
                if let Err(error) = result {
                    let _ = delegate.ivars().handle.emit_to(
                        "main",
                        "certificate-error",
                        format!("Nie zapisano zaufania certyfikatu: {error}"),
                    );
                    completion.call((Disposition::CancelAuthenticationChallenge, ptr::null_mut()));
                    return;
                }
            }
            delegate.ivars().session.borrow_mut().insert(
                session_key.clone(),
                valid_until.unwrap_or(now + 3600).min(now + 8 * 3600),
            );
            accept(&completion, &trust);
        });
        alert.beginSheetModalForWindow_completionHandler(&parent, Some(&callback));
    }
}

pub async fn install(
    view: &tauri::Webview,
    handle: &tauri::AppHandle,
    id: &str,
    name: &str,
    url: &tauri::Url,
) -> Result<(), String> {
    let scope = Scope {
        view_label: view.label().into(),
        app_id: id.into(),
        name: name.into(),
        host: url.host_str().ok_or("Brak hosta")?.to_lowercase(),
        port: url.port_or_known_default().ok_or("Brak portu")?,
    };
    let handle = handle.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    view.with_webview(move |platform| {
        let result = (|| -> Result<(), String> {
            let mtm = MainThreadMarker::new().ok_or("Delegate wymaga głównego wątku")?;
            let webview: &WKWebView = unsafe { &*platform.inner().cast() };
            let key = (&DELEGATE_KEY as *const u8).cast();
            let existing = unsafe {
                objc_getAssociatedObject((webview as *const WKWebView).cast_mut().cast(), key)
            };
            if !existing.is_null() {
                let delegate: &CertificateDelegate = unsafe { &*existing.cast() };
                *delegate.ivars().scope.borrow_mut() = scope;
                return Ok(());
            }
            let original =
                unsafe { webview.navigationDelegate() }.ok_or("Brak oryginalnego delegate")?;
            let allocated = mtm.alloc::<CertificateDelegate>().set_ivars(DelegateIvars {
                original: Weak::from_retained(&original),
                handle,
                scope: RefCell::new(scope),
                session: RefCell::new(HashMap::new()),
                prompting: Cell::new(false),
            });
            let delegate: Retained<CertificateDelegate> =
                unsafe { msg_send![super(allocated), init] };
            unsafe {
                // WKNavigationDelegate is weak; the associated object owns the
                // proxy for exactly as long as this WKWebView lives.
                objc_setAssociatedObject(
                    (webview as *const WKWebView).cast_mut().cast(),
                    key,
                    Retained::as_ptr(&delegate).cast_mut().cast(),
                    1,
                );
            }
            unsafe {
                webview.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            }
            Ok(())
        })();
        let _ = send.send(result);
    })
    .map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn forget_certificates(
    window: tauri::Webview,
    handle: tauri::AppHandle,
    state: tauri::State<'_, Store>,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    let uuid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.update(|c| {
        c.trusted_certificates.retain(|p| p.app_id != id);
        Ok(())
    })?;
    if let Some(view) = handle.get_webview(&format!("embedded-{uuid}")) {
        view.close().map_err(|e| e.to_string())?;
    }
    if let Some(view) = handle.get_webview_window(&format!("service-{uuid}")) {
        view.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_denials_and_fatal_errors_cannot_be_overridden_by_pins() {
        use objc2_security::SecTrustResultType as Result;
        assert!(can_offer_exception(0, Result::RecoverableTrustFailure));
        for result in [
            Result::Deny,
            Result::FatalTrustFailure,
            Result::OtherError,
            Result::Invalid,
        ] {
            assert!(!can_offer_exception(0, result));
        }
        assert!(!can_offer_exception(-1, Result::RecoverableTrustFailure));
    }

    #[test]
    fn certificate_exception_is_bound_to_app_host_port_leaf_and_expiry() {
        let scope = Scope {
            view_label: "embedded-test".into(),
            app_id: "app-1".into(),
            name: "Test".into(),
            host: "100.107.29.27".into(),
            port: 8006,
        };
        let pin = TrustedCertificate {
            app_id: scope.app_id.clone(),
            host: scope.host.clone(),
            port: scope.port,
            sha256: "fingerprint-A".into(),
            expires_at: 200,
        };
        assert!(pin_matches(&pin, &scope, "fingerprint-A", 100));
        assert!(!pin_matches(&pin, &scope, "fingerprint-B", 100));
        assert!(!pin_matches(&pin, &scope, "fingerprint-A", 200));
        assert!(!scope.matches("other-host", 8006));
        assert!(!scope.matches("100.107.29.27", 443));
        let other = Scope {
            app_id: "other-app".into(),
            ..scope
        };
        assert!(!pin_matches(&pin, &other, "fingerprint-A", 100));
    }
}
