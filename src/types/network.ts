export interface NetworkProfile {
  id: string;
  name?: string;
  interfaceType: "wifi" | "ethernet" | "other";
  ssid?: string;
  gatewayIp?: string;
  gatewayMac?: string;
  subnet?: string;
  trustedLocal: boolean;
  localOverride?: boolean | null;
  lastSeen: string;
}
