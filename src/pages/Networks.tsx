import { Wifi, EthernetPort, ShieldCheck } from "lucide-react";
import type { NetworkProfile } from "../types/network";
export function Networks({
  networks,
  current,
  onTrust,
  onForget,
}: {
  networks: NetworkProfile[];
  current: string | null;
  onTrust: (id: string, trusted: boolean) => void;
  onForget: (id: string) => void;
}) {
  return (
    <>
      <div className="info">
        Nowa sieć staje się Trusted LAN, jeśli odpowiadają co najmniej dwa różne
        lokalne endpointy. Możesz też wybrać LAN ręcznie przyciskiem „Zaufaj tej
        sieci”, również gdy macOS nie udostępnia MAC routera.
      </div>
      <div className="info">
        Gdy adres LAN działa w przeglądarce, ale tutaj wybierany jest Tailscale,
        sprawdź w macOS: Prywatność i ochrona → Sieć lokalna. Zezwól HomeLab na
        dostęp; przy uruchamianiu z VS Code sprawdź także Visual Studio Code.
      </div>
      <div className="networks">
        {networks.map((n) => (
          <article className="network-card" key={n.id}>
            <div className="network-heading">
              {n.interfaceType === "wifi" ? (
                <Wifi size={22} />
              ) : (
                <EthernetPort size={22} />
              )}
              <div>
                <h3>{n.name}</h3>
                <p>
                  {n.interfaceType}
                  {current === n.id && " · Aktualna sieć"}
                </p>
              </div>
              <span className={`status ${n.trustedLocal ? "lan" : "remote"}`}>
                <ShieldCheck size={13} />
                {n.trustedLocal
                  ? n.localOverride === true
                    ? "LAN · wybrany ręcznie"
                    : n.gatewayMac
                      ? "Trusted LAN"
                      : "LAN · weryfikowany"
                  : "Remote"}
              </span>
            </div>
            <dl>
              <dt>SSID</dt>
              <dd>{n.ssid ?? "Niedostępne"}</dd>
              <dt>Gateway</dt>
              <dd>{n.gatewayIp ?? "—"}</dd>
              <dt>Gateway MAC</dt>
              <dd>{n.gatewayMac ?? "Niedostępne"}</dd>
              <dt>Subnet / interfejs</dt>
              <dd>{n.subnet ?? "—"}</dd>
              <dt>Last seen</dt>
              <dd>{new Date(n.lastSeen).toLocaleString("pl-PL")}</dd>
            </dl>
            <footer>
              <button onClick={() => onTrust(n.id, !n.trustedLocal)}>
                {n.trustedLocal ? "Używaj Tailscale" : "Zaufaj tej sieci"}
              </button>
              <button onClick={() => onForget(n.id)}>Forget network</button>
            </footer>
          </article>
        ))}
      </div>
      {!networks.length && (
        <div className="empty">
          Nie wykryto sieci z domyślną bramą. Połącz się z Wi-Fi lub Ethernet i
          odśwież.
        </div>
      )}
    </>
  );
}
