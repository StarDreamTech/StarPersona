// Read-only smoke test: real listeners, HTTPS 204 probes, throwaway workspace.
// Run: node_modules/.bin/vite-node scripts/check-starport-import.mts
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { initialWorkspace, parseWorkspace } from "../src/model";
import { mergeNodes, snapshotKey, type Tested } from "../src/starport";
const exec = promisify(execFile);
const catalog = async () => {
  const { stdout } = await exec("/usr/bin/curl", [
    "-q",
    "-fsS",
    "--max-time",
    "12",
    "--max-filesize",
    "2097152",
    "--noproxy",
    "*",
    "http://127.0.0.1:18765/api/status?nodeIds=1",
  ]);
  const s = JSON.parse(stdout);
  assert.equal(s.pending, false);
  return s.exits
    .filter(
      (x: any) =>
        x.open &&
        x.port >= 1024 &&
        x.socks5h === `socks5h://127.0.0.1:${x.port}`,
    )
    .map((x: any) => ({
      name: x.name,
      port: x.port,
      nodeId: s.nodeIds?.[x.port] ?? null,
      ms: null,
    })) as Tested[];
};
const nodes = await catalog();
let cursor = 0;
await Promise.all(
  Array.from({ length: 4 }, async () => {
    while (cursor < nodes.length) {
      const n = nodes[cursor++];
      try {
        const { stdout } = await exec("/usr/bin/curl", [
          "-q",
          "-fsS",
          "--noproxy",
          "",
          "--proxy",
          `socks5h://127.0.0.1:${n.port}`,
          "--proto",
          "=https",
          "--max-time",
          "6",
          "--connect-timeout",
          "4",
          "--max-filesize",
          "1024",
          "-o",
          "/dev/null",
          "-w",
          "%{http_code} %{size_download} %{time_total}",
          "https://cp.cloudflare.com/generate_204",
        ]);
        const [code, bytes, time] = stdout.split(" ");
        if (code === "204" && bytes === "0")
          n.ms = Math.ceil(Number(time) * 1000);
      } catch {
        /* failed candidates stay unaccepted */
      }
    }
  }),
);
const latest = new Set((await catalog()).map(snapshotKey)),
  stable = nodes.filter((n) => latest.has(snapshotKey(n)));
const w = initialWorkspace(),
  at = new Date().toISOString();
const first = mergeNodes(w, stable, at);
assert.ok(first.added > 0);
const again = mergeNodes(w, stable, at);
assert.equal(again.added, 0);
const before = JSON.stringify(w);
mergeNodes(
  w,
  stable.map((n) => ({ ...n, ms: null })),
  at,
);
assert.equal(JSON.stringify(w), before);
assert.deepEqual(parseWorkspace(before), w);
console.log(
  JSON.stringify({
    listed: nodes.length,
    first,
    second: again,
    allFailedPreservesState: true,
  }),
);
