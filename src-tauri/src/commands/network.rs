use crate::{
    models::*,
    store::{main_only, validate_url, Store},
};
use futures::future::join_all;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

async fn output(program: &str, args: &[&str]) -> String {
    let result = tokio::time::timeout(
        Duration::from_millis(800),
        tokio::process::Command::new(program)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    match result {
        Ok(Ok(o)) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    }
}
fn field(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix(key).map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
}
fn parse_subnet(config: &str) -> Option<String> {
    let fields: Vec<_> = config
        .lines()
        .find(|l| l.trim().starts_with("inet "))?
        .split_whitespace()
        .collect();
    let address: std::net::Ipv4Addr = fields.get(1)?.parse().ok()?;
    let i = fields.iter().position(|s| *s == "netmask")?;
    let mask = u32::from_str_radix(fields.get(i + 1)?.trim_start_matches("0x"), 16).ok()?;
    Some(format!(
        "{}/{}",
        std::net::Ipv4Addr::from(u32::from(address) & mask),
        mask.count_ones()
    ))
}
// Match the actual router on the physical interface, ignoring incomplete ARP entries.
fn gateway_mac(arp: &str, gateway: &str, interface: &str) -> Option<String> {
    arp.lines().find_map(|line| {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.get(1)? != &format!("({gateway})")
            || parts.get(3)? == &"(incomplete)"
            || parts.get(4)? != &"on"
            || parts.get(5)? != &interface
        {
            return None;
        }
        let bytes: Option<Vec<u8>> = parts
            .get(3)?
            .split(':')
            .map(|part| u8::from_str_radix(part, 16).ok())
            .collect();
        let bytes = bytes?;
        if bytes.len() != 6 || bytes.iter().all(|b| *b == 0) || bytes[0] & 1 != 0 {
            return None;
        }
        Some(
            bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(":"),
        )
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NetworkIdentity {
    interface: String,
    gateway: String,
    mac: Option<String>,
    subnet: Option<String>,
}

impl NetworkIdentity {
    fn matches(&self, profile: &NetworkProfile) -> bool {
        (self.mac.is_some() || profile.local_override.is_some())
            && profile.id.split('|').next() == Some(self.interface.as_str())
            && profile.gateway_ip.as_deref() == Some(self.gateway.as_str())
            && profile.gateway_mac == self.mac
            && profile.subnet == self.subnet
    }
}

#[derive(Clone)]
pub struct LearnedNetworkCache {
    identity: NetworkIdentity,
    profile: NetworkProfile,
    updated: Instant,
}

// Only these fast read-only commands are needed to notice a network change.
// Wi-Fi metadata and endpoint learning run only on a cache miss.
async fn detect_identity() -> Option<NetworkIdentity> {
    let route = output("/sbin/route", &["-n", "get", "default"]).await;
    let interface = field(&route, "interface:")?;
    let gateway = field(&route, "gateway:")?;
    let arp_args = ["-an", "-i", interface.as_str()];
    let config_args = [interface.as_str()];
    let (arp, config) = tokio::join!(
        output("/usr/sbin/arp", &arp_args),
        output("/sbin/ifconfig", &config_args),
    );
    let mac = gateway_mac(&arp, &gateway, &interface);
    Some(NetworkIdentity {
        interface,
        gateway,
        mac,
        subnet: parse_subnet(&config),
    })
}

async fn detect(identity: &NetworkIdentity) -> NetworkProfile {
    let NetworkIdentity {
        interface,
        gateway,
        mac,
        subnet,
    } = identity;
    let hardware = output("/usr/sbin/networksetup", &["-listallhardwareports"]).await;
    let kind = hardware
        .split("Hardware Port: ")
        .find(|block| {
            block
                .lines()
                .any(|l| l.trim() == format!("Device: {interface}"))
        })
        .map(|b| {
            if b.starts_with("Wi-Fi") {
                "wifi"
            } else if b.to_lowercase().contains("ethernet") {
                "ethernet"
            } else {
                "other"
            }
        })
        .unwrap_or("other");
    let ssid = if kind == "wifi" {
        let s = output(
            "/usr/sbin/networksetup",
            &["-getairportnetwork", &interface],
        )
        .await;
        field(&s, "Current Wi-Fi Network:").filter(|s| s != "<redacted>")
    } else {
        None
    };
    // Missing router MAC requires live LAN verification rather than persistent trust.
    let id = format!(
        "{interface}|{kind}|{gateway}|{}|{}|{}",
        mac.as_deref().unwrap_or("unknown"),
        ssid.as_deref().unwrap_or(""),
        subnet.as_deref().unwrap_or("")
    );
    NetworkProfile {
        id,
        name: Some(
            ssid.clone()
                .unwrap_or_else(|| format!("{interface} · {gateway}")),
        ),
        interface_type: kind.into(),
        ssid,
        gateway_ip: Some(gateway.clone()),
        gateway_mac: mac.clone(),
        subnet: subnet.clone(),
        trusted_local: false,
        local_override: None,
        last_seen: chrono::Utc::now().to_rfc3339(),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Probe {
    Available,
    Unavailable,
    Unconfirmed,
}

async fn probe(raw: &str) -> Probe {
    let Ok(url) = validate_url(raw) else {
        return Probe::Unconfirmed;
    };
    let (Some(host), Some(port)) = (url.host_str(), url.port_or_known_default()) else {
        return Probe::Unconfirmed;
    };
    // Timeouts, DNS failures and macOS permission/routing errors do not prove that a service is offline.
    tokio::time::timeout(Duration::from_millis(400), async {
        let Ok(addresses) = tokio::net::lookup_host((host, port)).await else {
            return Probe::Unconfirmed;
        };
        let checks = addresses.take(4).map(|address| async move {
            match tokio::net::TcpStream::connect(address).await {
                Ok(_) => Probe::Available,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                    Probe::Unavailable
                }
                Err(_) => Probe::Unconfirmed,
            }
        });
        let results = join_all(checks).await;
        if results.contains(&Probe::Available) {
            Probe::Available
        } else if !results.is_empty() && results.iter().all(|p| *p == Probe::Unavailable) {
            Probe::Unavailable
        } else {
            Probe::Unconfirmed
        }
    })
    .await
    .unwrap_or(Probe::Unconfirmed)
}

#[cfg(test)]
async fn reachable(raw: &str) -> bool {
    probe(raw).await == Probe::Available
}

fn apply_health(resolution: &mut Resolution, probe: Probe, loaded_in_webview: bool) {
    match probe {
        Probe::Available => resolution.available = Some(true),
        Probe::Unavailable => {
            resolution.available = Some(false);
            resolution.status = "Offline".into();
            resolution.reason = "Serwer odrzucił połączenie z tym portem. Możesz spróbować otworzyć usługę ponownie.".into();
        }
        Probe::Unconfirmed if loaded_in_webview => {
            resolution.available = Some(true);
            resolution.reason = "Dostęp potwierdzony przez załadowaną stronę w HomeLab.".into();
        }
        Probe::Unconfirmed => {
            resolution.available = None;
            resolution.reason =
                "Szybki test nie potwierdził dostępności. Otwórz usługę, aby sprawdzić połączenie."
                    .into();
        }
    }
}

// Public URLs and Tailscale addresses in Local URL must never establish LAN trust.
fn on_local_subnet(address: std::net::IpAddr, subnet: Option<&str>) -> bool {
    let std::net::IpAddr::V4(address) = address else {
        return false;
    };
    let Some((network, prefix)) = subnet.and_then(|s| s.split_once('/')) else {
        return false;
    };
    let (Ok(network), Ok(prefix)) = (network.parse::<std::net::Ipv4Addr>(), prefix.parse::<u32>())
    else {
        return false;
    };
    if !address.is_private() || prefix == 0 || prefix > 32 {
        return false;
    }
    let mask = u32::MAX << (32 - prefix);
    u32::from(address) & mask == u32::from(network) & mask
}

async fn reachable_on_lan(host: &str, port: u16, identity: &NetworkIdentity) -> bool {
    tokio::time::timeout(Duration::from_millis(400), async {
        let Ok(addresses) = tokio::net::lookup_host((host, port)).await else {
            return false;
        };
        let checks = addresses
            .filter(|a| on_local_subnet(a.ip(), identity.subnet.as_deref()))
            .take(4)
            .map(|address| async move {
                // An overlapping VPN route must not count as access through the home LAN.
                let route = output("/sbin/route", &["-n", "get", &address.ip().to_string()]).await;
                field(&route, "interface:").as_deref() == Some(identity.interface.as_str())
                    && tokio::net::TcpStream::connect(address).await.is_ok()
            });
        join_all(checks).await.into_iter().any(|ok| ok)
    })
    .await
    .unwrap_or(false)
}

fn needs_learning(profile: &NetworkProfile) -> bool {
    profile.local_override.is_none() && (!profile.trusted_local || profile.gateway_mac.is_none())
}

fn cached_network(
    state: &Store,
    identity: &NetworkIdentity,
) -> Result<Option<NetworkProfile>, String> {
    let cache = state.network_cache.lock().map_err(|e| e.to_string())?;
    Ok(cache
        .as_ref()
        .filter(|cached| {
            cached.identity == *identity && cached.updated.elapsed() < Duration::from_secs(30)
        })
        .map(|cached| cached.profile.clone()))
}

pub async fn learned(state: &Store) -> Result<Option<NetworkProfile>, String> {
    let Some(identity) = detect_identity().await else {
        *state.network_cache.lock().map_err(|e| e.to_string())? = None;
        return Ok(None);
    };
    // Do not queue a tile click behind background learning when the current
    // gateway/interface/MAC/subnet have already been resolved.
    if let Some(profile) = cached_network(state, &identity)? {
        return Ok(Some(profile));
    }
    let _lock = state.resolver_lock.lock().await;
    if let Some(profile) = cached_network(state, &identity)? {
        return Ok(Some(profile));
    }
    let config = state.get()?;
    // Previously learned gateway identities can be reused immediately after
    // restart too; SSID/hardware metadata is not needed to choose their route.
    let mut net = if let Some(known) = config.networks.iter().find(|n| identity.matches(n)) {
        let mut known = known.clone();
        known.last_seen = chrono::Utc::now().to_rfc3339();
        known
    } else {
        detect(&identity).await
    };
    if let Some(known) = config.networks.iter().find(|n| n.id == net.id) {
        net.trusted_local = known.trusted_local;
        net.name = known.name.clone();
        net.local_override = known.local_override;
    }
    if needs_learning(&net) {
        let endpoints: HashSet<_> = config
            .apps
            .iter()
            .filter_map(|a| validate_url(&a.local_url).ok())
            .filter_map(|u| Some((u.host_str()?.to_string(), u.port_or_known_default()?)))
            .collect();
        let checks = endpoints
            .iter()
            .take(6)
            .map(|(host, port)| reachable_on_lan(host, *port, &identity));
        net.trusted_local = join_all(checks).await.into_iter().filter(|x| *x).count() >= 2;
    }
    state.update(|c| {
        c.networks.retain(|n| n.id != net.id);
        c.networks.push(net.clone());
        Ok(())
    })?;
    *state.network_cache.lock().map_err(|e| e.to_string())? = Some(LearnedNetworkCache {
        identity,
        profile: net.clone(),
        updated: Instant::now(),
    });
    Ok(Some(net))
}
pub fn choose(app: &HomelabApp, trusted: bool) -> Resolution {
    let local = app.connection_mode == "local" || (app.connection_mode == "auto" && trusted);
    let url = if local {
        Some(app.local_url.clone())
    } else {
        app.remote_url.clone()
    };
    Resolution {
        available: if url.is_none() { Some(false) } else { None },
        status: if url.is_none() {
            "Offline"
        } else if local {
            "LAN"
        } else {
            "Tailscale"
        }
        .into(),
        reason: if url.is_none() {
            "Brak adresu Tailscale. Dodaj go w Applications lub wybierz tryb Local."
        } else if local {
            "Dostęp lokalny"
        } else {
            "Dostęp zdalny"
        }
        .into(),
        url,
    }
}
#[tauri::command]
pub async fn current_network(
    window: tauri::Webview,
    state: tauri::State<'_, Store>,
) -> Result<Option<NetworkProfile>, String> {
    main_only(&window)?;
    learned(&state).await
}
#[tauri::command]
pub async fn resolve_app(
    window: tauri::Webview,
    state: tauri::State<'_, Store>,
    id: String,
) -> Result<Resolution, String> {
    main_only(&window)?;
    let net = learned(&state).await?;
    let c = state.get()?;
    let app = c
        .apps
        .iter()
        .find(|a| a.id == id)
        .ok_or("Aplikacja nie istnieje")?;
    Ok(choose(app, net.is_some_and(|n| n.trusted_local)))
}
#[derive(serde::Serialize)]
pub struct ServiceSnapshot {
    pub network: Option<NetworkProfile>,
    pub services: HashMap<String, Resolution>,
}
#[tauri::command]
pub async fn check_services(
    window: tauri::Webview,
    state: tauri::State<'_, Store>,
    views: tauri::State<'_, super::webview::ServiceViews>,
) -> Result<ServiceSnapshot, String> {
    main_only(&window)?;
    // An explicit refresh must also retry a failed decision after permissions change.
    // Tile openings still reuse the current decision without repeating probes.
    *state.network_cache.lock().map_err(|e| e.to_string())? = None;
    let network = learned(&state).await?;
    let trusted = network.as_ref().is_some_and(|n| n.trusted_local);
    let apps = state.get()?.apps;
    let views = &*views;
    let services = join_all(apps.into_iter().map(|app| async move {
        let mut resolution = choose(&app, trusted);
        if let Some(url) = resolution.url.clone() {
            let health = probe(&url).await;
            apply_health(&mut resolution, health, views.was_loaded(&app.id, &url));
        }
        (app.id, resolution)
    }))
    .await
    .into_iter()
    .collect();
    Ok(ServiceSnapshot { network, services })
}
#[tauri::command]
pub async fn trust_network(
    window: tauri::Webview,
    state: tauri::State<'_, Store>,
    id: String,
    trusted: bool,
) -> Result<(), String> {
    main_only(&window)?;
    let _lock = state.resolver_lock.lock().await;
    *state.network_cache.lock().map_err(|e| e.to_string())? = None;
    state.update(|c| {
        let n = c
            .networks
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or("Sieć nie istnieje")?;
        n.trusted_local = trusted;
        n.local_override = Some(trusted);
        Ok(())
    })
}
#[tauri::command]
pub async fn forget_network(
    window: tauri::Webview,
    state: tauri::State<'_, Store>,
    id: String,
) -> Result<(), String> {
    main_only(&window)?;
    let _lock = state.resolver_lock.lock().await;
    *state.network_cache.lock().map_err(|e| e.to_string())? = None;
    state.update(|c| {
        c.networks.retain(|n| n.id != id);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn learned_network_requires_current_gateway_mac_not_just_subnet() {
        let identity = NetworkIdentity {
            interface: "en0".into(),
            gateway: "192.168.0.1".into(),
            mac: Some("aa:bb:cc:dd:ee:ff".into()),
            subnet: Some("192.168.0.0/24".into()),
        };
        let mut profile = NetworkProfile {
            id: "en0|wifi|home".into(),
            name: None,
            interface_type: "wifi".into(),
            ssid: None,
            gateway_ip: Some(identity.gateway.clone()),
            gateway_mac: identity.mac.clone(),
            subnet: identity.subnet.clone(),
            trusted_local: true,
            local_override: None,
            last_seen: String::new(),
        };
        assert!(identity.matches(&profile));
        profile.gateway_mac = Some("11:22:33:44:55:66".into());
        assert!(!identity.matches(&profile));
        profile.gateway_mac = None;
        assert!(!identity.matches(&profile));
        let mut without_mac = identity.clone();
        without_mac.mac = None;
        assert!(!without_mac.matches(&profile));
        profile.local_override = Some(true);
        assert!(without_mac.matches(&profile));
        assert!(!needs_learning(&profile));
        without_mac.gateway = "192.168.1.1".into();
        assert!(!without_mac.matches(&profile));
        without_mac.gateway = identity.gateway.clone();
        without_mac.interface = "en1".into();
        assert!(!without_mac.matches(&profile));
    }

    #[test]
    fn failed_automatic_learning_can_recover_but_manual_remote_is_preserved() {
        let mut profile = NetworkProfile {
            id: "en0|wifi|home".into(),
            name: None,
            interface_type: "wifi".into(),
            ssid: None,
            gateway_ip: Some("192.168.0.1".into()),
            gateway_mac: Some("aa:bb:cc:dd:ee:ff".into()),
            subnet: Some("192.168.0.0/24".into()),
            trusted_local: false,
            local_override: None,
            last_seen: String::new(),
        };
        assert!(needs_learning(&profile));
        profile.local_override = Some(false);
        assert!(!needs_learning(&profile));
        profile.local_override = None;
        profile.trusted_local = true;
        assert!(!needs_learning(&profile));
        profile.trusted_local = false;
        profile.gateway_mac = None;
        assert!(needs_learning(&profile));
        profile.trusted_local = true;
        assert!(needs_learning(&profile));
    }

    #[test]
    fn only_private_addresses_on_current_subnet_can_establish_lan_trust() {
        let subnet = Some("192.168.0.0/24");
        for address in ["192.168.0.20", "192.168.0.50"] {
            assert!(on_local_subnet(address.parse().unwrap(), subnet));
        }
        for address in [
            "100.107.29.27",
            "8.8.8.8",
            "192.168.1.50",
            "127.0.0.1",
            "::1",
        ] {
            assert!(!on_local_subnet(address.parse().unwrap(), subnet));
        }
        assert!(!on_local_subnet(
            "8.8.8.8".parse().unwrap(),
            Some("0.0.0.0/0")
        ));
        assert!(!on_local_subnet("192.168.0.20".parse().unwrap(), None));
    }

    #[test]
    fn arp_parser_selects_router_interface_and_normalizes_mac() {
        let arp = "? (192.168.0.1) at (incomplete) on en0 ifscope [ethernet]\n\
                   ? (192.168.0.1) at 11:22:33:44:55:66 on utun3 ifscope [ethernet]\n\
                   ? (192.168.0.1) at 2:15:95:77:ef:b on en0 ifscope [ethernet]";
        assert_eq!(
            gateway_mac(arp, "192.168.0.1", "en0").as_deref(),
            Some("02:15:95:77:ef:0b")
        );
        assert!(gateway_mac(arp, "192.168.0.2", "en0").is_none());
    }

    #[tokio::test]
    #[ignore = "Explicit check of configured endpoints on the current home network; writes only to a temporary config"]
    async fn current_network_resolves_expected_route() {
        let path = std::path::PathBuf::from(
            std::env::var("HOMELAB_NETWORK_CHECK_CONFIG").expect("set config path explicitly"),
        );
        let config: Config = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let temp =
            std::env::temp_dir().join(format!("homelab-network-{}.json", uuid::Uuid::new_v4()));
        let state = Store {
            path: temp.clone(),
            config: std::sync::Mutex::new(config),
            network_cache: std::sync::Mutex::new(None),
            resolver_lock: tokio::sync::Mutex::new(()),
        };
        let result = learned(&state).await;
        let network = result.unwrap().expect("active physical network");
        let expected_lan = std::env::var("HOMELAB_EXPECT_LAN")
            .expect("set expected route explicitly")
            .parse::<bool>()
            .unwrap();
        assert_eq!(
            network.trusted_local, expected_lan,
            "unexpected route for the configured LAN endpoints"
        );
        for app in state
            .get()
            .unwrap()
            .apps
            .iter()
            .filter(|a| a.connection_mode == "auto")
        {
            assert_eq!(
                choose(app, network.trusted_local).url.as_deref(),
                if expected_lan {
                    Some(app.local_url.as_str())
                } else {
                    app.remote_url.as_deref()
                }
            );
        }
        // A saved positive decision without router MAC cannot bypass live checks.
        if network.gateway_mac.is_none() {
            state
                .update(|c| {
                    c.apps.clear();
                    Ok(())
                })
                .unwrap();
            *state.network_cache.lock().unwrap() = None;
            assert!(!learned(&state).await.unwrap().unwrap().trusted_local);
        }
        let _ = std::fs::remove_file(temp);
    }

    #[test]
    fn fingerprint_subnet_is_stable_across_dhcp_changes() {
        assert_eq!(
            parse_subnet("  inet 192.168.0.52 netmask 0xffffff00 broadcast 192.168.0.255"),
            Some("192.168.0.0/24".into())
        );
        assert_eq!(
            parse_subnet("  inet 192.168.0.99 netmask 0xffffff00 broadcast 192.168.0.255"),
            Some("192.168.0.0/24".into())
        );
    }
    #[test]
    fn resolution_respects_modes_and_never_guesses_local_on_unknown_network() {
        let mut a = crate::models::seed().apps.remove(0);
        assert_eq!(choose(&a, false).status, "Offline");
        assert_eq!(choose(&a, true).url.as_deref(), Some(a.local_url.as_str()));
        a.remote_url = Some("http://lab.tail.example:8123".into());
        assert_eq!(choose(&a, false).status, "Tailscale");
        a.connection_mode = "remote".into();
        assert_eq!(choose(&a, true).url, a.remote_url);
        a.connection_mode = "local".into();
        assert_eq!(choose(&a, false).status, "LAN");
    }
    #[test]
    fn uncertain_tcp_result_does_not_mark_a_working_webview_offline() {
        let app = crate::models::seed().apps.remove(0);
        let mut result = choose(&app, true);
        apply_health(&mut result, Probe::Unconfirmed, false);
        assert_eq!(result.status, "LAN");
        assert_eq!(result.available, None);
        apply_health(&mut result, Probe::Unconfirmed, true);
        assert_eq!(result.status, "LAN");
        assert_eq!(result.available, Some(true));
        apply_health(&mut result, Probe::Unavailable, true);
        assert_eq!(result.status, "Offline");
        assert_eq!(result.available, Some(false));
    }

    #[tokio::test]
    async fn tcp_probe_distinguishes_open_and_closed_ports() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        assert!(reachable(&url).await);
        drop(listener);
        assert!(!reachable(&url).await);
    }
}
