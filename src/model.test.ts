import { describe, expect, it } from "vitest";
import {
  createProfile,
  initialWorkspace,
  parseWorkspace,
  removeGroup,
  removeProxy,
  stripDemoData,
  visibleProfiles,
  visibleProxies,
  type Workspace,
} from "./model";

/** The sample rows that StarPersona 0.1.0 seeded, kept only as a test fixture. */
function demo(): Workspace {
  const w = initialWorkspace();
  w.groups = [
    { id: "web3", name: "Web3 工作室", color: "#5375ed" },
    { id: "commerce", name: "跨境电商", color: "#8b6cdb" },
    { id: "social", name: "社媒运营", color: "#28a186" },
  ];
  w.proxies = [
    {
      id: "us",
      name: "美国 · 洛杉矶",
      protocol: "SOCKS5",
      host: "192.0.2.10",
      port: 1080,
      country: "US",
    },
    {
      id: "sg",
      name: "新加坡",
      protocol: "SOCKS5",
      host: "198.51.100.20",
      port: 1080,
      country: "SG",
    },
    {
      id: "jp",
      name: "日本 · 东京",
      protocol: "HTTP",
      host: "203.0.113.30",
      port: 8080,
      country: "JP",
    },
  ];
  const rows = [
    ["star-01-emily", "web3", "us", "Windows", "Web3,主账号", "日常工作环境", true],
    ["star-02-james", "web3", "us", "Windows", "Web3", "", false],
    ["star-03-sophia", "commerce", "sg", "macOS", "Amazon,主账号", "美国站店铺", true],
    ["star-04-oliver", "commerce", "jp", "Windows", "Shopify", "", false],
    ["star-05-charlotte", "social", "us", "macOS", "Instagram", "品牌内容运营", false],
    ["star-06-noah", "social", "sg", "Windows", "TikTok,测试", "", false],
    ["star-07-amelia", "web3", "", "Linux", "测试", "", false],
    ["star-08-liam", "", "", "Windows", "", "备用环境", false],
  ] as const;
  w.profiles = rows.map((row, index) => ({
    ...createProfile(w),
    id: `example-${index + 1}`,
    serial: index + 1,
    name: row[0],
    groupId: row[1],
    proxyId: row[2],
    os: row[3],
    tags: row[4] ? row[4].split(",") : [],
    notes: row[5],
    favorite: row[6],
    createdAt: `2026-11-${String(10 + Math.floor(index / 2)).padStart(2, "0")}T0${(index % 5) + 1}:30:00Z`,
  }));
  return w;
}

describe("workspace operations", () => {
  it("round-trips backups and rejects invalid references and duplicate IDs", () => {
    const w = demo();
    expect(parseWorkspace(JSON.stringify(w))).toEqual(w);
    w.profiles[0]!.groupId = "missing";
    expect(() => parseWorkspace(JSON.stringify(w))).toThrow("失效引用");
    const duplicate = demo();
    duplicate.profiles.push(duplicate.profiles[0]!);
    expect(() => parseWorkspace(JSON.stringify(duplicate))).toThrow("ID");
  });
  it("combines search, group, favorites and archive filters", () => {
    const w = demo();
    expect(visibleProfiles(w, "主账号", "", "all", "serial")).toHaveLength(2);
    expect(visibleProfiles(w, "", "web3", "favorite", "serial")).toHaveLength(
      1,
    );
    w.profiles[0]!.archived = true;
    expect(visibleProfiles(w, "", "", "all", "serial")).toHaveLength(7);
    expect(visibleProfiles(w, "emily", "", "archived", "serial")).toHaveLength(
      1,
    );
  });
  it("clears references when a group or proxy is removed", () => {
    const w = demo();
    removeGroup(w, "web3");
    removeProxy(w, "us");
    expect(
      w.profiles.some((p) => p.groupId === "web3" || p.proxyId === "us"),
    ).toBe(false);
    expect(() => parseWorkspace(JSON.stringify(w))).not.toThrow();
  });
  it("never reuses a serial number after archiving", () => {
    const w = demo();
    w.profiles[7]!.archived = true;
    expect(createProfile(w).serial).toBe(9);
  });
  it("starts empty and strips the sample rows older versions seeded", () => {
    const fresh = initialWorkspace();
    expect([
      fresh.profiles.length,
      fresh.proxies.length,
      fresh.groups.length,
    ]).toEqual([0, 0, 0]);
    const w = demo();
    w.profiles.push({
      ...createProfile(w),
      id: "real",
      groupId: "",
      proxyId: "",
    });
    expect(stripDemoData(w)).toBe(true);
    expect(w.profiles.map((p) => p.id)).toEqual(["real"]);
    expect(w.proxies).toHaveLength(0);
    expect(w.groups).toHaveLength(0);
    expect(stripDemoData(w)).toBe(false);
    expect(() => parseWorkspace(JSON.stringify(w))).not.toThrow();
  });
  it("keeps a sample group or proxy that a user still uses", () => {
    const w = demo();
    w.profiles = [
      { ...createProfile(w), id: "real", groupId: "web3", proxyId: "us" },
    ];
    stripDemoData(w);
    expect(w.profiles.map((p) => p.id)).toEqual(["real"]);
    expect(w.groups.map((g) => g.id)).toEqual(["web3"]);
    expect(w.proxies.map((p) => p.id)).toEqual(["us"]);
  });
});

describe("proxy filters", () => {
  it("matches name, host, port, protocol and country name", () => {
    const w = demo();
    expect(visibleProxies(w, "洛杉矶", "", "")).toHaveLength(1);
    expect(visibleProxies(w, "192.0.2.10", "", "")).toHaveLength(1);
    expect(visibleProxies(w, "8080", "", "")).toHaveLength(1);
    expect(visibleProxies(w, "美国", "", "", (c) => (c === "US" ? "美国" : c))).toHaveLength(1);
    expect(visibleProxies(w, "", "HTTP", "")).toHaveLength(1);
    expect(visibleProxies(w, "", "SOCKS5", "")).toHaveLength(2);
  });
  it("splits healthy, failed and untested entries", () => {
    const w = demo();
    expect(visibleProxies(w, "", "", "unknown")).toHaveLength(3);
    w.proxies[0]!.check = { at: "2026-11-18T00:00:00Z", ok: true, message: "" };
    w.proxies[1]!.check = {
      at: "2026-11-18T00:00:00Z",
      ok: false,
      message: "timeout",
    };
    expect(visibleProxies(w, "", "", "ok").map((p) => p.id)).toEqual(["us"]);
    expect(visibleProxies(w, "", "", "fail").map((p) => p.id)).toEqual(["sg"]);
    expect(visibleProxies(w, "", "", "unknown").map((p) => p.id)).toEqual([
      "jp",
    ]);
    expect(visibleProxies(w, "192.0.2.10", "SOCKS5", "ok")).toHaveLength(1);
  });
});

it('checks account aliases symmetrically on create and restore; old profiles stay readable', () => {
  const w = initialWorkspace(); const a = createProfile(w);
  Object.assign(a,{platform:'Portal',accountLabel:' QA ',checkDuplicate:true}); w.profiles.push(a);
  const b=createProfile(w); Object.assign(b,{platform:'portal',accountLabel:'qa',archived:true});w.profiles.push(b);
  expect(()=>parseWorkspace(JSON.stringify(w))).not.toThrow(); b.archived=false;
  expect(()=>parseWorkspace(JSON.stringify(w))).toThrow(/账号/); w.profiles.reverse(); expect(()=>parseWorkspace(JSON.stringify(w))).toThrow(/账号/);
});
it('validates launch URLs and requires an account identity for limits', () => {
  const w=initialWorkspace(), p=createProfile(w);w.profiles.push(p);
  for(const s of ['file:///etc', 'https://u:p@example.org/', '-private', 'http://a\n.test/']) {p.startUrls=[s];expect(()=>parseWorkspace(JSON.stringify(w))).toThrow(/启动网址/);}
  p.startUrls=['https://example.org/a','http://localhost:3000/'];expect(()=>parseWorkspace(JSON.stringify(w))).not.toThrow();
  p.exclusiveAccount=true;expect(()=>parseWorkspace(JSON.stringify(w))).toThrow(/账号/);
});
