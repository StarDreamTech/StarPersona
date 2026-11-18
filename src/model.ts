export type OS = "Windows" | "macOS" | "Linux";
export interface Group {
  id: string;
  name: string;
  color: string;
}
export interface Proxy {
  id: string;
  name: string;
  protocol: "SOCKS5" | "HTTP";
  host: string;
  port: number;
  country: string;
}
export interface Profile {
  id: string;
  serial: number;
  name: string;
  groupId: string;
  proxyId: string;
  os: OS;
  tags: string[];
  notes: string;
  createdAt: string;
  archived: boolean;
  favorite: boolean;
  language: string;
  timezone: string;
  resolution: string;
  fingerprint: string;
}
export interface Workspace {
  version: 1;
  profiles: Profile[];
  groups: Group[];
  proxies: Proxy[];
  settings: { name: string; defaultOs: OS; defaultTimezone: string };
}
export const uid = () => crypto.randomUUID();
export const colors = [
  "#5375ed",
  "#8b6cdb",
  "#28a186",
  "#d39443",
  "#d66d8c",
  "#5b9cc7",
];
export function createProfile(workspace: Workspace): Profile {
  const serial = Math.max(0, ...workspace.profiles.map((p) => p.serial)) + 1;
  return {
    id: uid(),
    serial,
    name: `新环境 ${String(serial).padStart(2, "0")}`,
    groupId: "",
    proxyId: "",
    os: workspace.settings.defaultOs,
    tags: [],
    notes: "",
    createdAt: new Date().toISOString(),
    archived: false,
    favorite: false,
    language: "zh-CN",
    timezone: workspace.settings.defaultTimezone,
    resolution: "1920 × 1080",
    fingerprint: uid().replaceAll("-", "").slice(0, 16),
  };
}
export function initialWorkspace(): Workspace {
  const workspace: Workspace = {
    version: 1,
    settings: {
      name: "我的工作空间",
      defaultOs: "Windows",
      defaultTimezone: "Asia/Shanghai",
    },
    groups: [
      { id: "web3", name: "Web3 工作室", color: colors[0]! },
      { id: "commerce", name: "跨境电商", color: colors[1]! },
      { id: "social", name: "社媒运营", color: colors[2]! },
    ],
    proxies: [
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
    ],
    profiles: [],
  };
  const examples = [
    [
      "star-01-emily",
      "web3",
      "us",
      "Windows",
      "Web3,主账号",
      "日常工作环境",
      true,
    ],
    ["star-02-james", "web3", "us", "Windows", "Web3", "", false],
    [
      "star-03-sophia",
      "commerce",
      "sg",
      "macOS",
      "Amazon,主账号",
      "美国站店铺",
      true,
    ],
    ["star-04-oliver", "commerce", "jp", "Windows", "Shopify", "", false],
    [
      "star-05-charlotte",
      "social",
      "us",
      "macOS",
      "Instagram",
      "品牌内容运营",
      false,
    ],
    ["star-06-noah", "social", "sg", "Windows", "TikTok,测试", "", false],
    ["star-07-amelia", "web3", "", "Linux", "测试", "", false],
    ["star-08-liam", "", "", "Windows", "", "备用环境", false],
  ] as const;
  workspace.profiles = examples.map((row, index) => ({
    ...createProfile(workspace),
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
  return workspace;
}
export function visibleProfiles(
  workspace: Workspace,
  query: string,
  group: string,
  tab: string,
  sort: string,
): Profile[] {
  const search = query.trim().toLocaleLowerCase();
  return workspace.profiles
    .filter((p) => {
      const groupName =
        workspace.groups.find((g) => g.id === p.groupId)?.name ?? "未分组";
      const proxy = workspace.proxies.find((proxy) => proxy.id === p.proxyId);
      return (
        p.archived === (tab === "archived") &&
        (!group || p.groupId === group || (group === "__none" && !p.groupId)) &&
        (tab !== "favorite" || p.favorite) &&
        (!search ||
          [p.name, groupName, p.notes, proxy?.host ?? "", ...p.tags]
            .join(" ")
            .toLocaleLowerCase()
            .includes(search))
      );
    })
    .sort((a, b) =>
      sort === "name"
        ? a.name.localeCompare(b.name)
        : sort === "newest"
          ? b.createdAt.localeCompare(a.createdAt)
          : a.serial - b.serial,
    );
}
export function removeGroup(workspace: Workspace, id: string) {
  workspace.groups = workspace.groups.filter((g) => g.id !== id);
  workspace.profiles.forEach((p) => {
    if (p.groupId === id) p.groupId = "";
  });
}
export function removeProxy(workspace: Workspace, id: string) {
  workspace.proxies = workspace.proxies.filter((p) => p.id !== id);
  workspace.profiles.forEach((p) => {
    if (p.proxyId === id) p.proxyId = "";
  });
}
export function parseWorkspace(text: string): Workspace {
  const value: unknown = JSON.parse(text);
  if (!value || typeof value !== "object")
    throw new Error("备份文件格式不正确");
  const w = value as Workspace;
  if (
    w.version !== 1 ||
    !Array.isArray(w.profiles) ||
    !Array.isArray(w.groups) ||
    !Array.isArray(w.proxies) ||
    !w.settings ||
    typeof w.settings.name !== "string" ||
    !w.settings.name.trim() ||
    w.settings.name.length > 80 ||
    !["Windows", "macOS", "Linux"].includes(w.settings.defaultOs) ||
    typeof w.settings.defaultTimezone !== "string" ||
    !w.settings.defaultTimezone.trim() ||
    w.settings.defaultTimezone.length > 100 ||
    w.profiles.length > 5000 ||
    w.groups.length > 500 ||
    w.proxies.length > 5000
  )
    throw new Error("不是受支持的星相工作空间备份");
  const validId = (id: unknown) =>
    typeof id === "string" && id.trim().length > 0 && id.length <= 100;
  const unique = (items: { id: string }[]) =>
    items.every((i) => i && typeof i === "object" && validId(i.id)) &&
    new Set(items.map((i) => i.id)).size === items.length;
  if (![w.groups, w.proxies, w.profiles].every(unique))
    throw new Error("备份包含重复或无效的 ID");
  if (
    w.groups.some(
      (g) =>
        typeof g.name !== "string" ||
        !g.name.trim() ||
        g.name.length > 80 ||
        !/^#[0-9a-f]{6}$/i.test(g.color),
    )
  )
    throw new Error("分组数据不正确");
  if (
    w.proxies.some(
      (p) =>
        !["SOCKS5", "HTTP"].includes(p.protocol) ||
        typeof p.name !== "string" ||
        !p.name.trim() ||
        p.name.length > 80 ||
        typeof p.host !== "string" ||
        !p.host.trim() ||
        p.host.length > 253 ||
        /\s/.test(p.host) ||
        !Number.isInteger(p.port) ||
        p.port < 1 ||
        p.port > 65535 ||
        typeof p.country !== "string" ||
        p.country.length > 8,
    )
  )
    throw new Error("代理配置不正确");
  if (
    w.profiles.some(
      (p) =>
        typeof p.name !== "string" ||
        !p.name.trim() ||
        p.name.length > 80 ||
        !Number.isSafeInteger(p.serial) ||
        p.serial < 1 ||
        !["Windows", "macOS", "Linux"].includes(p.os) ||
        typeof p.archived !== "boolean" ||
        typeof p.favorite !== "boolean" ||
        !Array.isArray(p.tags) ||
        p.tags.length > 12 ||
        p.tags.some((t) => typeof t !== "string" || t.length > 30) ||
        typeof p.notes !== "string" ||
        p.notes.length > 1000 ||
        typeof p.createdAt !== "string" ||
        !Number.isFinite(Date.parse(p.createdAt)) ||
        [p.language, p.timezone, p.resolution, p.fingerprint].some(
          (s) => typeof s !== "string" || !s || s.length > 100,
        ) ||
        typeof p.groupId !== "string" ||
        (p.groupId && !w.groups.some((g) => g.id === p.groupId)) ||
        typeof p.proxyId !== "string" ||
        (p.proxyId && !w.proxies.some((proxy) => proxy.id === p.proxyId)),
    )
  )
    throw new Error("环境数据不完整或包含失效引用");
  if (new Set(w.profiles.map((p) => p.serial)).size !== w.profiles.length)
    throw new Error("环境序号不能重复");
  return w;
}
