import { invoke, isTauri } from "@tauri-apps/api/core";
export const desktop = isTauri();
export async function native<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!desktop)
    throw new Error(
      "Ta funkcja wymaga aplikacji desktopowej. Uruchom npm run tauri dev.",
    );
  return invoke<T>(command, args);
}
