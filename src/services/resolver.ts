import type { NetworkProfile } from "../types/network";
import type { Resolution } from "../types/app";
import { native } from "./native";
export const resolveApp = (id: string) =>
  native<Resolution>("resolve_app", { id });
export type OpenTarget = "embedded" | "window" | "external";
export const openApp = (id: string, target: OpenTarget = "embedded") =>
  native<Resolution>("open_app", { id, target });
export const reloadService = (id: string) =>
  native<Resolution>("reload_service", { id });
export const closeService = () => native<void>("close_service");
export const checkServices = () =>
  native<{
    services: Record<string, Resolution>;
    network: NetworkProfile | null;
  }>("check_services");

export const forgetCertificates = (id: string) =>
  native<void>("forget_certificates", { id });
