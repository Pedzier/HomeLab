export interface HomelabApp {
  id: string;
  name: string;
  icon?: string;
  localUrl: string;
  remoteUrl?: string;
  localHost?: string;
  localPort?: number;
  remoteHost?: string;
  remotePort?: number;
  connectionMode: "auto" | "local" | "remote";
  credentialId?: string;
  order: number;
}
export type ServiceStatus = "LAN" | "Tailscale" | "Offline";
export interface Resolution {
  available?: boolean | null;
  url: string | null;
  status: ServiceStatus;
  reason: string;
}
