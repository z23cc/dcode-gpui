import { writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const tsCheckout = resolve(process.argv[2] ?? "");
const protocolSource = (path: string) => join(tsCheckout, "protocol/src", path);

const { hostRpcRegistry, hostStreamRpcRegistry } = await import(protocolSource("host/registry.ts"));
const { RELEASED_FLOOR_METHOD_NAMES } = await import(protocolSource("host/released-floor.ts"));
const { splitConnectionManifest, SERVES_EVERY_INSTALLED_MAJOR } = await import(
  protocolSource("framework/index.ts")
);
const { buildStreamManifest } = await import(protocolSource("framework/stream-compat.ts"));

const rpc = splitConnectionManifest(
  hostRpcRegistry,
  RELEASED_FLOOR_METHOD_NAMES,
  SERVES_EVERY_INSTALLED_MAJOR,
);
const stream = buildStreamManifest(hostStreamRpcRegistry, SERVES_EVERY_INSTALLED_MAJOR);

const output = join(import.meta.dir, "../crates/dcode-client/manifest.json");
writeFileSync(output, JSON.stringify({ rpc, stream }, null, 2) + "\n");
console.log(
  `rpc floor ${Object.keys(rpc.manifest).length}, rpc optional ${Object.keys(rpc.optionalManifest).length}, stream ${Object.keys(stream).length} -> ${output}`,
);
