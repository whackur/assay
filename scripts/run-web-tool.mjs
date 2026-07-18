import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const [tool, ...args] = process.argv.slice(2);
const binaries = {
  eslint: path.join(root, "web", "node_modules", "eslint", "bin", "eslint.js"),
  next: path.join(root, "web", "node_modules", "next", "dist", "bin", "next"),
};
const binary = binaries[tool];
if (!binary) {
  console.error("run-web-tool supports only next and eslint");
  process.exit(2);
}
const result = spawnSync(process.execPath, [binary, ...args], {
  stdio: "inherit",
  env: { ...process.env, NEXT_TELEMETRY_DISABLED: "1" },
});
process.exit(result.status ?? 1);
