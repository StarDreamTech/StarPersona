import { uid, type Proxy, type Workspace } from "./model";
export interface Node {
  name: string;
  port: number;
  nodeId: string | null;
}
export interface Listing {
  nodes: Node[];
  skipped: number;
}
export interface Measure {
  ms: number;
  bytesPerSecond: number | null;
}
export interface Tested extends Node {
  ms: number | null;
}
export const endpoint = (p: Pick<Proxy, "protocol" | "host" | "port">) =>
  `${p.protocol}:${p.host
    .toLowerCase()
    .replace(/\.$/, "")
    .replace(/^localhost$/, "127.0.0.1")}:${p.port}`;
const local = (n: Node) =>
  endpoint({ protocol: "SOCKS5", host: "127.0.0.1", port: n.port });
export const snapshotKey = (n: Node) =>
  JSON.stringify([n.port, n.nodeId, n.name]);
export const health = (p: Proxy) =>
  !p.check
    ? "未检测"
    : p.check.ok
      ? `可用${p.check.lastGoodMs != null ? ` · ${p.check.lastGoodMs} ms` : ""}`
      : `失败 · ${p.check.message}`;

// Only candidates passing HTTPS in this run can modify a binding. Keep a working
// existing listener when multiple healthy listeners represent the same upstream.
export function mergeNodes(
  w: Workspace,
  rows: Tested[],
  at: string,
  inUse = new Set<string>(),
) {
  const count = {
    added: 0,
    updated: 0,
    unchanged: 0,
    failed: 0,
    conflict: 0,
    inUse: 0,
  };
  const matches = (p: Proxy, n: Node) =>
    n.nodeId
      ? p.sourceNode === n.nodeId
      : !p.sourceNode && p.sourceName === n.name;
  const saved = new Map(w.proxies.map((p) => [endpoint(p), p]));
  const existing = new Set(
    rows
      .filter((n) => {
        const p = saved.get(local(n));
        return p && matches(p, n);
      })
      .map(snapshotKey),
  );
  const sorted = [...rows].sort(
    (a, b) =>
      Number(existing.has(snapshotKey(b))) -
        Number(existing.has(snapshotKey(a))) || a.port - b.port,
  );
  const seen = new Set<string>();
  for (const n of sorted) {
    if (n.ms === null) {
      count.failed++;
      continue;
    }
    // An old server has no upstream identity. Do not infer one from a name or public IP.
    const key = n.nodeId ?? local(n);
    if (seen.has(key)) {
      count.unchanged++;
      continue;
    }
    const same = n.nodeId
      ? w.proxies.find((p) => p.sourceNode === n.nodeId)
      : undefined;
    const occupied = w.proxies.filter((p) => endpoint(p) === local(n));
    const slot = occupied[0];
    if (
      occupied.length > 1 ||
      (slot &&
        ((same && slot !== same) ||
          (slot.sourceNode && slot.sourceNode !== n.nodeId) ||
          (!slot.sourceNode && slot.sourceName && slot.sourceName !== n.name)))
    ) {
      count.conflict++;
      continue;
    }
    const p = same ?? slot;
    if (!p) {
      if (n.nodeId && w.proxies.some((p) => p.id === n.nodeId)) {
        count.conflict++;
        continue;
      }
      w.proxies.push({
        id: n.nodeId ?? uid(),
        name: n.name,
        country: "",
        protocol: "SOCKS5",
        host: "127.0.0.1",
        port: n.port,
        sourceName: n.name,
        ...(n.nodeId ? { sourceNode: n.nodeId } : {}),
        check: { at, ok: true, message: "", lastGoodMs: n.ms },
      });
      count.added++;
    } else {
      const moved = endpoint(p) !== local(n);
      if (moved && inUse.has(p.id)) {
        count.inUse++;
        continue;
      }
      const changed =
        moved || p.sourceName !== n.name || (p.sourceNode ?? null) !== n.nodeId;
      if (moved) {
        p.host = "127.0.0.1";
        p.port = n.port;
        p.protocol = "SOCKS5";
        delete p.check;
      }
      if (p.name === p.sourceName) p.name = n.name;
      p.sourceName = n.name;
      if (n.nodeId) p.sourceNode = n.nodeId;
      p.check = { ...p.check, ok: true, at, message: "", lastGoodMs: n.ms };
      if (changed) count.updated++;
      else count.unchanged++;
    }
    seen.add(key);
  }
  return count;
}
