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
  sourceNode?: string;
  sourceName?: string;
  check?: {
    at: string;
    ok: boolean;
    message: string;
    lastGoodMs?: number;
    bytesPerSecond?: number;
    speedAt?: string;
  };
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
  platform?: string;
  accountLabel?: string;
  checkDuplicate?: boolean;
  exclusiveAccount?: boolean;
  startUrls?: string[];
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
  return {
    version: 1,
    settings: {
      name: "我的工作空间",
      defaultOs: "Windows",
      defaultTimezone: "Asia/Shanghai",
    },
    groups: [],
    proxies: [],
    profiles: [],
  };
}
const demoProxyIds = ["us", "sg", "jp"];
const demoGroupIds = ["web3", "commerce", "social"];
const reservedHosts = ["192.0.2.", "198.51.100.", "203.0.113."];
export const isDemoProxy = (p: Proxy) =>
  demoProxyIds.includes(p.id) &&
  !p.sourceNode &&
  reservedHosts.some((host) => p.host.startsWith(host));
const isDemoProfile = (p: Profile) => /^example-\d+$/.test(p.id);
/** Drops the sample rows that earlier versions seeded, keeping user data. */
export function stripDemoData(workspace: Workspace): boolean {
  const profiles = workspace.profiles.filter((p) => !isDemoProfile(p));
  const usedProxies = new Set(profiles.map((p) => p.proxyId));
  const proxies = workspace.proxies.filter(
    (p) => !isDemoProxy(p) || usedProxies.has(p.id),
  );
  const usedGroups = new Set(profiles.map((p) => p.groupId));
  const groups = workspace.groups.filter(
    (g) => !demoGroupIds.includes(g.id) || usedGroups.has(g.id),
  );
  if (
    profiles.length === workspace.profiles.length &&
    proxies.length === workspace.proxies.length &&
    groups.length === workspace.groups.length
  )
    return false;
  workspace.profiles = profiles;
  workspace.proxies = proxies;
  workspace.groups = groups;
  return true;
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
          [p.name, groupName, p.notes, p.platform, p.accountLabel, proxy?.host ?? "", ...p.tags]
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
export function visibleProxies(
  workspace: Workspace,
  query: string,
  protocol: string,
  status: string,
  countryName: (code: string) => string = (code) => code,
): Proxy[] {
  const search = query.trim().toLocaleLowerCase();
  return workspace.proxies.filter((p) => {
    const text = [
      p.name,
      p.host,
      String(p.port),
      p.protocol,
      countryName(p.country),
      p.country,
      p.sourceName ?? "",
    ]
      .join(" ")
      .toLocaleLowerCase();
    return (
      (!protocol || p.protocol === protocol) &&
      (!status ||
        (status === "ok"
          ? p.check?.ok === true
          : status === "fail"
            ? Boolean(p.check) && p.check?.ok !== true
            : !p.check)) &&
      (!search || text.includes(search))
    );
  });
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
        p.country.length > 8 ||
        (p.sourceNode !== undefined &&
          (typeof p.sourceNode !== "string" ||
            !/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(
              p.sourceNode,
            ))) ||
        (p.sourceName !== undefined &&
          (typeof p.sourceName !== "string" ||
            !p.sourceName.trim() ||
            p.sourceName.length > 80)) ||
        (p.check !== undefined &&
          (!p.check ||
            typeof p.check.ok !== "boolean" ||
            typeof p.check.at !== "string" ||
            !p.check.at ||
            p.check.at.length > 40 ||
            typeof p.check.message !== "string" ||
            p.check.message.length > 100 ||
            (p.check.speedAt !== undefined &&
              (typeof p.check.speedAt !== "string" ||
                !p.check.speedAt ||
                p.check.speedAt.length > 40)) ||
            [p.check.lastGoodMs, p.check.bytesPerSecond].some(
              (v) => v !== undefined && (!Number.isSafeInteger(v) || v < 0),
            ))),
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
  validateAccountRules(w.profiles);
  return w;
}

export function validateAccountRules(profiles: Profile[]) {
  const keys = new Map<string, boolean>();
  for (const p of profiles) {
    for (const s of [p.platform, p.accountLabel]) if (s !== undefined && (typeof s !== 'string' || s.length > 100 || /[\u0000-\u001f\u007f]/.test(s))) throw Error('平台和账号备注限 100 字，不可含控制字符');
    for (const b of [p.checkDuplicate, p.exclusiveAccount]) if (b !== undefined && typeof b !== 'boolean') throw Error('账号限制设置无效');
    if (p.startUrls !== undefined && (!Array.isArray(p.startUrls) || p.startUrls.length > 8 || !p.startUrls.every(s => {
      if (typeof s !== 'string' || !/^https?:\/\//i.test(s) || s.length > 2048 || /[\s\\\u0000-\u001f\u007f]/u.test(s)) return false;
      try { const u = new URL(s); return !!u.host && !u.username && !u.password; } catch { return false; }
    }))) throw Error('启动网址最多 8 条，仅支持不含账号密码的 HTTP(S) 地址，每条不超过 2048 字');
    const pair = [p.platform?.trim().toLowerCase(), p.accountLabel?.trim().toLowerCase()];
    if (pair.some(x => !x)) { if (p.checkDuplicate || p.exclusiveAccount) throw Error('启用账号限制需填写平台和账号备注'); continue; }
    if (p.archived) continue;
    const key = JSON.stringify(pair);
    if (keys.has(key) && (keys.get(key) || p.checkDuplicate)) throw Error('同一平台已存在相同账号备注，请修改或关闭查重');
    keys.set(key, !!p.checkDuplicate);
  }
}
