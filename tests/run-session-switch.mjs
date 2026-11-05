import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { rolldown } from "rolldown";

const here = dirname(fileURLToPath(import.meta.url));
const outfile = "/tmp/session-switch.test.mjs";

const bundle = await rolldown({
  input: join(here, "session-switch.test.ts"),
  platform: "node",
  external: (id) => id.startsWith("node:"),
  plugins: [
    {
      name: "fake-tauri",
      resolveId(id) {
        if (id === "@tauri-apps/api/core") return join(here, "fake-tauri.ts");
        return undefined;
      },
    },
  ],
});

await bundle.write({ file: outfile, format: "esm" });
await bundle.close();

const child = spawn(process.execPath, ["--test", outfile], { stdio: "inherit" });
child.on("exit", (code) => {
  process.exit(code ?? 1);
});
