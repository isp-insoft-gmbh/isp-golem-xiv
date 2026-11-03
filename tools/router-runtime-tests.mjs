import { createRequire } from "node:module";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = process.env.MINIFLARE_ROOT;
if (!root) throw new Error("MINIFLARE_ROOT missing; run through the pinned mise task");
const require = createRequire(import.meta.url);
// npm global prefixes differ: Windows uses node_modules, Unix lib/node_modules.
const miniflare = require.resolve("miniflare", { paths: [root, resolve(root, "lib")] });
const repository = fileURLToPath(new URL("../", import.meta.url));
for (const script of ["router/tests/runtime.mjs", "router/tests/worker.mjs"]) {
  const result = spawnSync(process.execPath, [script, miniflare, "router/build"], {
    cwd: repository,
    stdio: "inherit",
    timeout: 60000,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
