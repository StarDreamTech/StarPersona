import { describe, expect, it } from "vitest";
import { initialWorkspace, parseWorkspace } from "./model";
import { mergeNodes, type Tested } from "./starport";
const node = (change: Partial<Tested> = {}): Tested => ({
  name: "sample route",
  port: 18001,
  nodeId: "11111111-1111-8111-8111-111111111111",
  ms: 125,
  ...change,
});
function empty() {
  return { ...initialWorkspace(), proxies: [], profiles: [] } as ReturnType<
    typeof initialWorkspace
  >;
}
describe("incremental StarPort import", () => {
  it("is idempotent, detects aliases, and keeps the already used healthy listener", () => {
    const w = empty();
    mergeNodes(w, [node({ port: 18002 })], "a");
    expect(
      mergeNodes(w, [node(), node({ port: 18002, name: "renamed" })], "b"),
    ).toMatchObject({ added: 0, updated: 1, unchanged: 1 });
    expect(w.proxies).toHaveLength(1);
    expect(w.proxies[0]).toMatchObject({ port: 18002, name: "renamed" });
    expect(parseWorkspace(JSON.stringify(w))).toEqual(w);
  });
  it("retains user labels, stable binding IDs, and last good metrics on bad or absent rows", () => {
    const w = empty();
    mergeNodes(w, [node()], "a");
    const p = w.proxies[0]!;
    p.name = "my label";
    const original = JSON.stringify(w);
    mergeNodes(w, [node({ port: 18003, ms: null })], "b");
    mergeNodes(w, [], "b");
    expect(JSON.stringify(w)).toBe(original);
    mergeNodes(w, [node({ port: 18003, name: "new alias" })], "c");
    expect(p).toMatchObject({
      id: node().nodeId,
      port: 18003,
      name: "my label",
    });
  });
  it("does not move a listener used by a running profile", () => {
    const w = empty();
    mergeNodes(w, [node()], "a");
    const old = JSON.stringify(w);
    expect(
      mergeNodes(w, [node({ port: 18002 })], "b", new Set([w.proxies[0]!.id]))
        .inUse,
    ).toBe(1);
    expect(JSON.stringify(w)).toBe(old);
  });
  it("does not steal an occupied endpoint from another node", () => {
    const w = empty();
    mergeNodes(w, [node()], "a");
    const before = JSON.stringify(w);
    const c = mergeNodes(
      w,
      [node({ nodeId: "22222222-2222-8222-8222-222222222222" })],
      "b",
    );
    expect(c.conflict).toBe(1);
    expect(JSON.stringify(w)).toBe(before);
  });
  it("adopts a matching manual endpoint without breaking profile references", () => {
    const w = empty();
    w.proxies.push({
      id: "manual",
      name: "custom",
      host: "localhost",
      port: 18001,
      protocol: "SOCKS5",
      country: "JP",
    });
    mergeNodes(w, [node()], "a");
    expect(w.proxies[0]).toMatchObject({
      id: "manual",
      name: "custom",
      country: "JP",
      sourceNode: node().nodeId,
    });
    expect(w.proxies).toHaveLength(1);
  });
  it("legacy server dedups by endpoint, not display name; refuses unsafe reassignment", () => {
    const w = empty();
    const a = node({ nodeId: null });
    mergeNodes(w, [a, a, { ...a, port: 18002 }], "a");
    expect(w.proxies).toHaveLength(2);
    expect(mergeNodes(w, [{ ...a, name: "other" }], "b").conflict).toBe(1);
  });
});
