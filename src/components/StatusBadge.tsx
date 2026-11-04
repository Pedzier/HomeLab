import type { ServiceStatus } from "../types/app";
export function StatusBadge({
  status,
  available,
  reason,
}: {
  status?: ServiceStatus;
  available?: boolean | null;
  reason?: string;
}) {
  return (
    <span
      title={reason}
      className={`status ${available == null && status !== "Offline" ? "pending" : status === "LAN" ? "lan" : status === "Tailscale" ? "remote" : status === "Offline" ? "offline" : "pending"}`}
    >
      <i />
      {status ?? "Sprawdzanie…"}
    </span>
  );
}
