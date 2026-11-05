import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export default function globalSetup(): void {
  if (process.env.OXRAY_E2E !== "1") {
    return;
  }
  const root = fileURLToPath(new URL("../..", import.meta.url));
  execFileSync("cargo", ["xtask", "seed", "--url", "http://127.0.0.1:4174"], {
    cwd: root,
    stdio: "inherit",
  });
}
