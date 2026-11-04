import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { delimiter, resolve } from "node:path";
const env = { ...process.env };
// Also use the isolated toolchain installed during initial PoC setup, if available.
const isolated = "/private/tmp/homelab-cargo";
if (
  spawnSync("cargo", ["--version"], { stdio: "ignore" }).status !== 0 &&
  existsSync(`${isolated}/bin/cargo`)
) {
  env.CARGO_HOME = isolated;
  env.RUSTUP_HOME = "/private/tmp/homelab-rustup";
  env.PATH = `${isolated}/bin${delimiter}${env.PATH ?? ""}`;
}
const [mode, ...args] = process.argv.slice(2);
const command = mode === "cargo" ? "cargo" : resolve("node_modules/.bin/tauri");
const child = spawnSync(command, args, { env, stdio: "inherit" });
if (child.error) console.error(child.error.message);
process.exit(child.status ?? 1);
