import { copyFileSync, existsSync } from "node:fs";
const destination = new URL("../src-tauri/sandbox-check/endpoints.local.json", import.meta.url);
if (!existsSync(destination)) {
  copyFileSync(new URL("../src-tauri/sandbox-check/endpoints.example.json", import.meta.url), destination);
  console.log("Created isolated sandbox endpoints; edit src-tauri/sandbox-check/endpoints.local.json before testing your LAN.");
}
