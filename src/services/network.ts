import { native } from "./native";
import type { NetworkProfile } from "../types/network";
export const currentNetwork = () =>
  native<NetworkProfile | null>("current_network");
export const trustNetwork = (id: string, trusted: boolean) =>
  native<void>("trust_network", { id, trusted });
export const forgetNetwork = (id: string) =>
  native<void>("forget_network", { id });
