import type { HomelabApp } from "../types/app";
import type { Credential } from "../types/credential";
import type { NetworkProfile } from "../types/network";
import { native } from "./native";
export interface Config {
  apps: HomelabApp[];
  credentials: Credential[];
  networks: NetworkProfile[];
}
export const loadConfig = () => native<Config>("load_config");
export const saveApp = (app: HomelabApp) => native<void>("save_app", { app });
export const deleteApp = (id: string) => native<void>("delete_app", { id });
export const reorderApps = (ids: string[]) =>
  native<void>("reorder_apps", { ids });
