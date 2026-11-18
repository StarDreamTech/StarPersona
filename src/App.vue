<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import {
  Archive,
  ArrowDown,
  ArrowDownToLine,
  ArrowRight,
  ArrowUpDown,
  Check,
  CheckCheck,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  Copy,
  Database,
  Download,
  Ellipsis,
  ExternalLink,
  Fingerprint,
  Folder,
  FolderInput,
  FolderOpen,
  Github,
  Globe2,
  Grid2X2,
  HardDrive,
  Info,
  LayoutDashboard,
  List,
  LoaderCircle,
  Monitor,
  Plus,
  Puzzle,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Square,
  Star,
  Trash2,
  Upload,
  X,
  Zap,
} from "lucide-vue-next";
import {
  colors,
  createProfile,
  parseWorkspace,
  removeGroup,
  removeProxy,
  uid,
  visibleProfiles,
  visibleProxies,
  type Group,
  type Profile,
  type Proxy,
  type Workspace,
} from "./model";
import {
  commit,
  desktop,
  loadWorkspace,
  reloadWorkspace,
  ready,
  saving,
  storageError,
  workspace,
} from "./workspace";
import KernelVersions from "./KernelVersions.vue";
import Bookmarks from "./Bookmarks.vue";
import { BookMarked } from "lucide-vue-next";
import ProxyNetwork from "./ProxyNetwork.vue";
import ExtensionCenter from "./ExtensionCenter.vue";
import { refreshExtensions } from "./extensions";
import { endpoint, health } from "./starport";
const proxyNet = ref<InstanceType<typeof ProxyNetwork> | null>(null);
import { browser, refreshBrowser, saveBrowserSettings, toggleBrowserProcess } from "./browser";

type Page =
  "profiles" | "overview" | "groups" | "proxies" | "extensions" | "bookmarks" | "settings" | "archived";
type Modal =
  | ""
  | "profile"
  | "group"
  | "proxy"
  | "kernel"
  | "help"
  | "restore"
  | "deleteGroup"
  | "deleteProxy";
const page = ref<Page>("profiles");
const modal = ref<Modal>("");
const query = ref("");
const groupFilter = ref("");
const proxyQuery = ref("");
const proxyProtocol = ref("");
const proxyStatus = ref("");
const tab = ref("all");
const sort = ref("serial");
const pageSize = ref(10);
const pageNumber = ref(1);
const view = ref("list");
const selected = ref<string[]>([]);
const sidebarCollapsed = ref(false);
const showGroups = ref(false);
const showFilters = ref(false);
const menu = ref("");
const rowMenuPosition = ref({ left: "0px", top: "0px" });
const profileTab = ref("basic");
const draft = ref<Profile | null>(null);
const groupDraft = ref<Group>({ id: "", name: "", color: colors[0]! });
const proxyDraft = ref<Proxy>({
  id: "",
  name: "",
  protocol: "SOCKS5",
  host: "",
  port: 1080,
  country: "US",
});
const tagsInput = ref("");
const startPagesInput = ref("");
const editId = ref("");
const formError = ref("");
const toast = ref("");
const toastKind = ref<"success" | "info">("success");
let toastTimer: ReturnType<typeof setTimeout>;
const searchInput = ref<HTMLInputElement | null>(null);
const importInput = ref<HTMLInputElement | null>(null);
const restoreDraft = ref<Workspace | null>(null);
const settingsDraft = ref({ ...workspace.value.settings });
const kernelDraft = ref({ executable: "", addons: [] as string[] });
const updating=ref(false);
async function upgrade(){if(updating.value)return;updating.value=true;try{const {invoke}=await import("@tauri-apps/api/core");notify(await invoke<string>("brew_upgrade"));}catch(e){notify(String(e),"info")}finally{updating.value=false}}
const browserBusy = ref<string[]>([]);
let browserTimer: ReturnType<typeof setInterval>;
let unlistenBrowser: (() => void) | undefined;
let unlistenWorkspace: (() => void) | undefined;
let pollingBrowser = false;
const instanceFor = (id: string) => browser.value.instances.find((p) => p.profileId === id);
const browserLabel = (id: string) => instanceFor(id)?.stopping ? "关闭中" : instanceFor(id) ? "运行中" : browser.value.errors[id] ? "已退出" : "未启动";
async function pollBrowser() {
  if (!desktop || pollingBrowser) return;
  pollingBrowser = true;
  try {
    const previous = browser.value.errors;
    await refreshBrowser();
    for (const [id, message] of Object.entries(browser.value.errors)) {
      if (previous[id] !== message) notify(message, "info");
    }
  } catch (error) {
    if (modal.value === "kernel") formError.value = String(error);
  } finally { pollingBrowser = false; }
}
async function toggleBrowser(profile: Profile) {
  if (!desktop) { openModal("kernel"); return; }
  if (browserBusy.value.includes(profile.id) || instanceFor(profile.id)?.stopping) return;
  browserBusy.value.push(profile.id);
  const running = Boolean(instanceFor(profile.id));
  try {
    await toggleBrowserProcess(profile.id);
    notify(running ? "已请求关闭，正在等待浏览器保存数据" : "已启动独立浏览器进程");
  } catch (error) { notify(String(error), "info"); }
  finally { browserBusy.value = browserBusy.value.filter((id) => id !== profile.id); }
}
async function chooseKernel() {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({ title: "选择 Camoufox 应用或可执行文件", multiple: false });
  if (path) kernelDraft.value.executable = path;
}
async function saveKernel() {
  try {
    await saveBrowserSettings({ ...kernelDraft.value, addons: [...kernelDraft.value.addons] });
    closeModal(); notify("内核路径已保存");
  } catch (error) { formError.value = String(error); }
}
const activeProfiles = computed(() =>
  workspace.value.profiles.filter((p) => !p.archived),
);
const archivedProfiles = computed(() =>
  workspace.value.profiles.filter((p) => p.archived),
);
const favoriteCount = computed(
  () => activeProfiles.value.filter((p) => p.favorite).length,
);
const proxyRows = computed(() =>
  visibleProxies(
    workspace.value,
    proxyQuery.value,
    proxyProtocol.value,
    proxyStatus.value,
    (code) => countryNames[code] ?? code,
  ),
);
const proxyFiltered = computed(
  () =>
    proxyRows.value.length !== workspace.value.proxies.length ||
    Boolean(proxyQuery.value || proxyProtocol.value || proxyStatus.value),
);
const filtered = computed(() =>
  visibleProfiles(
    workspace.value,
    query.value,
    groupFilter.value,
    page.value === "archived" ? "archived" : tab.value,
    sort.value,
  ),
);
const pageCount = computed(() =>
  Math.max(1, Math.ceil(filtered.value.length / pageSize.value)),
);
const rows = computed(() =>
  filtered.value.slice(
    (pageNumber.value - 1) * pageSize.value,
    pageNumber.value * pageSize.value,
  ),
);
const allSelected = computed(
  () =>
    rows.value.length > 0 &&
    rows.value.every((p) => selected.value.includes(p.id)),
);
const titles: Record<Page, string> = {
  profiles: "浏览器窗口",
  overview: "工作台",
  groups: "分组管理",
  proxies: "代理 IP",
  extensions: "扩展中心",
  bookmarks: "共享书签",
  settings: "系统设置",
  archived: "回收站",
};
const groupsFor = (id: string) =>
  workspace.value.groups.find((g) => g.id === id);
const proxyFor = (id: string) =>
  workspace.value.proxies.find((p) => p.id === id);
const groupCount = (id: string) =>
  activeProfiles.value.filter((p) => p.groupId === id).length;
const countryNames: Record<string, string> = {
  US: "美国",
  SG: "新加坡",
  JP: "日本",
  HK: "中国香港",
  GB: "英国",
  DE: "德国",
  CN: "中国",
  CA: "加拿大",
};
const countryFlag = (country: string) =>
  /^[A-Z]{2}$/.test(country)
    ? String.fromCodePoint(...[...country].map((c) => c.charCodeAt(0) + 127397))
    : "🌐";
const formatDate = (date: string) =>
  new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(date));
const notify = (message: string, kind: "success" | "info" = "success") => {
  toast.value = message;
  toastKind.value = kind;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.value = "";
  }, 3600);
};
async function openGithub() {
  if (!desktop) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("open_github");
  } catch (cause) {
    notify(String(cause), "info");
  }
}
function navigate(target: Page) {
  page.value = target;
  selected.value = [];
  query.value = "";
  groupFilter.value = "";
  tab.value = "all";
  menu.value = "";
  if (target === "settings")
    settingsDraft.value = { ...workspace.value.settings };
}
function openModal(value: Modal) {
  formError.value = "";
  menu.value = "";
  modal.value = value;
  if (value === "kernel") kernelDraft.value = { ...browser.value.settings, addons: [...browser.value.settings.addons] };
}
function closeModal() {
  modal.value = "";
  formError.value = "";
}
function toggleProfileMenu(id: string, event: MouseEvent) {
  if (menu.value === id) {
    menu.value = "";
    return;
  }
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  rowMenuPosition.value = {
    left: `${Math.max(8, rect.right - 170)}px`,
    top: `${rect.bottom + 170 > window.innerHeight ? Math.max(8, rect.top - 160) : rect.bottom + 5}px`,
  };
  menu.value = id;
}
function editProfile(profile?: Profile) {
  if (profile && instanceFor(profile.id)) { notify("请先关闭环境，再编辑配置", "info"); return; }
  editId.value = profile?.id ?? "";
  draft.value = profile
    ? structuredClone(JSON.parse(JSON.stringify(profile)))
    : createProfile(workspace.value);
  if (!profile && groupFilter.value && groupFilter.value !== "__none")
    draft.value!.groupId = groupFilter.value;
  startPagesInput.value = (draft.value!.startUrls ?? []).join("\n");
  tagsInput.value = draft.value!.tags.join(", ");
  profileTab.value = "basic";
  openModal("profile");
  nextTick(() =>
    document.querySelector<HTMLInputElement>("#profile-name")?.focus(),
  );
}
async function saveProfile() {
  if (!draft.value) return;
  const profile = {
    ...draft.value,
    name: draft.value.name.trim(),
    platform: draft.value.platform?.trim() || '', accountLabel: draft.value.accountLabel?.trim() || '',
    startUrls: startPagesInput.value.split(/\r?\n/).map(v => v.trim()).filter(Boolean),
    tags: [
      ...new Set(
        tagsInput.value
          .split(/[,，]/)
          .map((t) => t.trim())
          .filter(Boolean),
      ),
    ],
  };
  if (!profile.name) {
    formError.value = "请填写环境名称";
    profileTab.value = "basic";
    return;
  }
  if (profile.tags.length > 12 || profile.tags.some((t) => t.length > 30)) {
    formError.value = "最多添加 12 个标签，每个不超过 30 个字";
    profileTab.value = "basic";
    return;
  }
  const ok = await commit((w) => {
    const index = w.profiles.findIndex((p) => p.id === editId.value);
    if (index >= 0) w.profiles[index] = profile;
    else {
      profile.serial = Math.max(0, ...w.profiles.map((p) => p.serial)) + 1;
      w.profiles.push(profile);
    }
  });
  if (ok) {
    closeModal();
    notify(editId.value ? "环境配置已保存" : "新环境已创建");
    tab.value = "all";
  } else formError.value = storageError.value;
}
async function duplicateProfile(profile: Profile) {
  if (
    await commit((w) => {
      const copy = {
        ...JSON.parse(JSON.stringify(profile)),
        ...{
          id: uid(),
          serial: createProfile(w).serial,
          accountLabel: "", checkDuplicate: false, exclusiveAccount: false,
          name: `${profile.name.slice(0, 75)} 副本`,
          createdAt: new Date().toISOString(),
          fingerprint: uid().replaceAll("-", "").slice(0, 16),
          archived: false,
        },
      };
      w.profiles.push(copy);
    })
  )
    notify("已复制环境，副本不沿用原账号备注和限制");
  menu.value = "";
}
async function favorite(profile: Profile) {
  await commit((w) => {
    const p = w.profiles.find((p) => p.id === profile.id);
    if (p) p.favorite = !p.favorite;
  });
}
async function archive(ids: string[], archived = true) {
  if (archived && ids.some((id) => instanceFor(id))) { notify("请先关闭选中的运行环境，再移入回收站", "info"); return; }
  if (
    await commit((w) =>
      w.profiles.forEach((p) => {
        if (ids.includes(p.id)) p.archived = archived;
      }),
    )
  ) {
    selected.value = [];
    menu.value = "";
    notify(
      archived
        ? `已将 ${ids.length} 个环境移入回收站，可随时恢复`
        : `已恢复 ${ids.length} 个环境`,
    );
  }
}
function toggleSelection(id: string) {
  selected.value = selected.value.includes(id)
    ? selected.value.filter((s) => s !== id)
    : [...selected.value, id];
}
function toggleAll() {
  selected.value = allSelected.value
    ? selected.value.filter((id) => !rows.value.some((p) => p.id === id))
    : [...new Set([...selected.value, ...rows.value.map((p) => p.id)])];
}
async function moveSelected(groupId: string) {
  if (
    await commit((w) =>
      w.profiles.forEach((p) => {
        if (selected.value.includes(p.id)) p.groupId = groupId;
      }),
    )
  )
    notify(`已移动 ${selected.value.length} 个环境`);
  selected.value = [];
  menu.value = "";
}
function editGroup(group?: Group) {
  editId.value = group?.id ?? "";
  groupDraft.value = group
    ? { ...group }
    : {
        id: uid(),
        name: "",
        color: colors[workspace.value.groups.length % colors.length]!,
      };
  openModal("group");
}
async function saveGroup() {
  const group = { ...groupDraft.value, name: groupDraft.value.name.trim() };
  if (!group.name) {
    formError.value = "请填写分组名称";
    return;
  }
  if (
    workspace.value.groups.some(
      (g) => g.id !== group.id && g.name === group.name,
    )
  ) {
    formError.value = "分组名称已存在";
    return;
  }
  if (
    await commit((w) => {
      const index = w.groups.findIndex((g) => g.id === group.id);
      if (index >= 0) w.groups[index] = group;
      else w.groups.push(group);
    })
  ) {
    closeModal();
    notify("分组已保存");
  } else formError.value = storageError.value;
}
function resetProxyFilters() {
  proxyQuery.value = "";
  proxyProtocol.value = "";
  proxyStatus.value = "";
}
function editProxy(proxy?: Proxy) {
  editId.value = proxy?.id ?? "";
  proxyDraft.value = proxy
    ? { ...proxy }
    : {
        id: uid(),
        name: "",
        protocol: "SOCKS5",
        host: "",
        port: 1080,
        country: "US",
      };
  openModal("proxy");
}
async function saveProxy() {
  const proxy = {
    ...proxyDraft.value,
    name: proxyDraft.value.name.trim(),
    host: proxyDraft.value.host.trim(),
  };
  if (
    !proxy.name ||
    !proxy.host ||
    /\s/.test(proxy.host) ||
    !Number.isInteger(proxy.port) ||
    proxy.port < 1 ||
    proxy.port > 65535
  ) {
    formError.value = "请填写名称、有效的主机地址及 1–65535 范围内的端口";
    return;
  }
  if (
    await commit((w) => {
      const index = w.proxies.findIndex((p) => p.id === proxy.id);
      if(w.proxies.some(p=>p.id!==proxy.id && endpoint(p)===endpoint(proxy))) throw new Error("相同协议、主机和端口的代理已存在");
      if(index>=0 && endpoint(w.proxies[index]!)!==endpoint(proxy)){delete proxy.sourceNode;delete proxy.sourceName;delete proxy.check}
      if (index >= 0) w.proxies[index] = proxy;
      else w.proxies.push(proxy);
    })
  ) {
    closeModal();
    notify("代理已保存，环境下次启动时应用");
  } else formError.value = storageError.value;
}
async function deleteEntity() {
  const isGroup = modal.value === "deleteGroup";
  if (
    await commit((w) =>
      isGroup ? removeGroup(w, editId.value) : removeProxy(w, editId.value),
    )
  ) {
    closeModal();
    notify(
      isGroup
        ? "分组已删除，原环境已移至未分组"
        : "代理已删除，关联环境已切换为直连",
    );
  }
}
async function exportBackup() {
  try {
    const content = JSON.stringify(workspace.value, null, 2);
    if (desktop) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      const path = await save({
        title: "导出工作空间",
        defaultPath: `starpersona-${new Date().toISOString().slice(0, 10)}.json`,
        filters: [{ name: "StarPersona 备份", extensions: ["json"] }],
      });
      if (!path) return;
      await writeTextFile(path, content);
    } else {
      const url = URL.createObjectURL(
        new Blob([content], { type: "application/json" }),
      );
      const link = document.createElement("a");
      link.href = url;
      link.download = `starpersona-${new Date().toISOString().slice(0, 10)}.json`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    }
    notify("工作空间备份已导出");
  } catch (e) {
    notify(`导出失败：${String(e)}`, "info");
  }
  menu.value = "";
}
async function chooseImport() {
  menu.value = "";
  if (!desktop) {
    importInput.value?.click();
    return;
  }
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { readTextFile, stat } = await import("@tauri-apps/plugin-fs");
    const path = await open({
      title: "选择工作空间备份",
      multiple: false,
      filters: [{ name: "StarPersona 备份", extensions: ["json"] }],
    });
    if (!path) return;
    if ((await stat(path)).size > 10 * 1024 * 1024)
      throw new Error("备份文件不能超过 10 MB");
    prepareRestore(await readTextFile(path));
  } catch (e) {
    notify(`导入失败：${String(e)}`, "info");
  }
}
function prepareRestore(text: string) {
  restoreDraft.value = parseWorkspace(text);
  openModal("restore");
}
async function importFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  try {
    if (file.size > 10 * 1024 * 1024) throw new Error("备份文件不能超过 10 MB");
    prepareRestore(await file.text());
  } catch (e) {
    notify(`导入失败：${String(e)}`, "info");
  }
  input.value = "";
}
async function restoreBackup() {
  if (
    restoreDraft.value &&
    (await commit((w) => Object.assign(w, restoreDraft.value)))
  ) {
    closeModal();
    navigate("profiles");
    notify("工作空间已恢复");
  }
}
async function saveSettings() {
  const settings = {
    ...settingsDraft.value,
    name: settingsDraft.value.name.trim(),
  };
  if (!settings.name) {
    notify("请填写工作空间名称", "info");
    return;
  }
  if (
    await commit((w) => {
      w.settings = settings;
    })
  )
    notify("工作空间设置已保存");
}
function keyboard(event: KeyboardEvent) {
  if (event.key === "Escape") {
    if (modal.value) closeModal();
    else menu.value = "";
  }
  if ((event.metaKey || event.ctrlKey) && event.key === "k") {
    event.preventDefault();
    if (page.value !== "profiles") navigate("profiles");
    nextTick(() => searchInput.value?.focus());
  }
  if (
    event.key.toLowerCase() === "n" &&
    !event.metaKey &&
    !event.ctrlKey &&
    !event.altKey &&
    !modal.value &&
    ready.value &&
    !(event.target as HTMLElement).closest(
      "input, textarea, select, [contenteditable]",
    )
  ) {
    event.preventDefault();
    editProfile();
  }
  if (modal.value && event.key === "Tab") {
    const elements = [
      ...document.querySelectorAll<HTMLElement>(
        '[role="dialog"] button:not(:disabled), [role="dialog"] input:not(:disabled), [role="dialog"] select, [role="dialog"] textarea, [role="dialog"] a[href]',
      ),
    ].filter((e) => e.offsetParent !== null);
    const first = elements[0];
    const last = elements.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }
}
function documentClick(event: MouseEvent) {
  if (!(event.target as HTMLElement).closest("[data-menu]")) menu.value = "";
}
let previousFocus: HTMLElement | null = null;
watch(modal, async (value) => {
  if (value) {
    previousFocus = document.activeElement as HTMLElement;
    await nextTick();
    document
      .querySelector<HTMLElement>(
        '[role="dialog"] input, [role="dialog"] button',
      )
      ?.focus();
  } else previousFocus?.focus();
});
watch([query, groupFilter, tab, pageSize, sort], () => {
  pageNumber.value = 1;
  selected.value = [];
});
watch(pageCount, (count) => {
  pageNumber.value = Math.min(pageNumber.value, count);
});
onMounted(async () => {
  await loadWorkspace();
  await pollBrowser();
  void refreshExtensions().catch(() => undefined);
  browserTimer = setInterval(pollBrowser, 1500);
  if (desktop) {
    const { listen } = await import("@tauri-apps/api/event");
    unlistenBrowser = await listen("browser-close-blocked", () => notify("请先关闭运行中的浏览器，再退出星相", "info"));
    unlistenWorkspace = await listen("workspace-changed", () => { void reloadWorkspace(); });
  }
  document.addEventListener("keydown", keyboard);
  document.addEventListener("click", documentClick);
});
onBeforeUnmount(() => {
  clearTimeout(toastTimer);
  clearInterval(browserTimer);
  unlistenBrowser?.();
  unlistenWorkspace?.();
  document.removeEventListener("keydown", keyboard);
  document.removeEventListener("click", documentClick);
});
</script>

<template>
  <div
    class="app-shell"
    :class="{ 'sidebar-collapsed': sidebarCollapsed, 'is-desktop': desktop }"
  >
    <aside class="sidebar" :inert="!!modal">
      <div class="native-drag" data-tauri-drag-region></div>
      <a
        href="#"
        class="brand"
        aria-label="星相工作空间"
        @click.prevent="navigate('profiles')"
      >
        <span class="brand-mark"
          ><Sparkles :size="23" :stroke-width="1.7"
        /></span>
        <span class="brand-text">星相<span>StarPersona</span></span
        ><span class="version">BETA</span>
      </a>
      <button class="workspace-switch" @click="navigate('settings')">
        <span class="workspace-icon"><Grid2X2 :size="17" /></span
        ><span>{{ workspace.settings.name }}<small>本地工作空间</small></span
        ><ChevronDown :size="15" />
      </button>
      <nav aria-label="主导航">
        <button
          :class="['nav-item', { active: page === 'overview' }]"
          aria-label="工作台"
          @click="navigate('overview')"
        >
          <LayoutDashboard :size="19" /><span>工作台</span>
        </button>
        <div class="nav-label">环境管理</div>
        <button
          :class="['nav-item', { active: page === 'profiles' }]"
          aria-label="浏览器窗口"
          @click="navigate('profiles')"
        >
          <Monitor :size="19" /><span>浏览器窗口</span
          ><span class="nav-count">{{ activeProfiles.length }}</span>
        </button>
        <button
          :class="['nav-item', { active: page === 'groups' }]"
          aria-label="分组管理"
          @click="navigate('groups')"
        >
          <Folder :size="19" /><span>分组管理</span>
        </button>
        <button
          :class="['nav-item', { active: page === 'proxies' }]"
          aria-label="代理 IP"
          @click="navigate('proxies')"
        >
          <Globe2 :size="19" /><span>代理 IP</span>
        </button>
        <button
          :class="['nav-item', { active: page === 'extensions' }]"
          aria-label="扩展中心"
          @click="navigate('extensions')"
        >
          <Puzzle :size="19" /><span>扩展中心</span>
        </button>
        <button :class="['nav-item', {active:page==='bookmarks'}]" @click="navigate('bookmarks')"><BookMarked :size="19"/><span>共享书签</span></button>
        <button
          :class="['nav-item', { active: page === 'archived' }]"
          aria-label="回收站"
          @click="navigate('archived')"
        >
          <Archive :size="19" /><span>回收站</span
          ><span v-if="archivedProfiles.length" class="nav-count muted">{{
            archivedProfiles.length
          }}</span>
        </button>
        <div class="nav-label groups-label">
          我的分组<button
            class="icon-button"
            title="创建分组"
            aria-label="创建分组"
            @click="editGroup()"
          >
            <Plus :size="14" />
          </button>
        </div>
        <button
          v-for="group in workspace.groups.slice(0, 6)"
          :key="group.id"
          class="nav-group"
          :class="{ chosen: page === 'profiles' && groupFilter === group.id }"
          @click="
            navigate('profiles');
            groupFilter = group.id;
          "
        >
          <i :style="{ background: group.color }"></i
          ><span>{{ group.name }}</span
          ><small>{{ groupCount(group.id) }}</small>
        </button>
      </nav>
      <div class="sidebar-bottom">
        <div class="local-card">
          <span class="local-card-icon"><ShieldCheck :size="19" /></span>
          <div>独立环境，安心工作<small>数据保存在你的设备上</small></div>
          <span class="live-dot"></span>
        </div>
        <button
          :class="['nav-item', { active: page === 'settings' }]"
          aria-label="系统设置"
          @click="navigate('settings')"
        >
          <Settings2 :size="19" /><span>系统设置</span>
        </button>
        <button class="nav-item" aria-label="使用帮助" @click="openModal('help')">
          <CircleHelp :size="19" /><span>使用帮助</span
          ><ExternalLink class="nav-end" :size="13" />
        </button>
        <div class="sidebar-footer">
          <span>StarPersona <small>v0.2.0</small></span
          ><span class="footer-dot">本地版</span>
        </div>
      </div>
      <button
        class="collapse-sidebar"
        :title="sidebarCollapsed ? '展开侧栏' : '收起侧栏'"
        :aria-label="sidebarCollapsed ? '展开侧栏' : '收起侧栏'"
        @click="sidebarCollapsed = !sidebarCollapsed"
      >
        <ChevronLeft :size="13" :class="{ rotated: sidebarCollapsed }" />
      </button>
    </aside>

    <div class="main-shell" :inert="!!modal">
      <header class="topbar" data-tauri-drag-region>
        <div class="topbar-message" data-tauri-drag-region>
          <span class="tiny-star">✧</span
          ><span data-tauri-drag-region>每一个身份，都有独立的空间。</span
          ><span class="topbar-tag">为专注而生</span>
        </div>
        <div class="topbar-actions">
          <span class="save-status"
            ><span
              class="live-dot"
              :class="{ pending: saving || !ready }"
            ></span
            >{{
              !ready ? "正在读取" : saving ? "正在保存" : "已保存到本地"
            }}</span
          ><span class="topbar-separator"></span
          ><button
            class="icon-button"
            title="GitHub"
            aria-label="打开 GitHub 仓库"
            @click="openGithub"
          >
            <Github :size="18" /></button
          ><button
            class="icon-button"
            title="使用帮助"
            aria-label="使用帮助"
            @click="openModal('help')"
          >
            <CircleHelp :size="19" /></button
          ><button
            class="user-avatar"
            title="工作空间设置"
            aria-label="工作空间设置"
            @click="navigate('settings')"
          >
            星
          </button>
        </div>
      </header>

      <main>
        <div class="page-heading">
          <div>
            <div class="eyebrow">
              WORKSPACE <ChevronRight :size="11" />
              {{ page === "archived" ? "回收站" : page === "extensions" ? "扩展" : "环境管理" }}
            </div>
            <h1>
              {{ titles[page]
              }}<span v-if="page === 'profiles'" class="heading-count">{{
                activeProfiles.length
              }}</span>
            </h1>
            <p>
              {{
                page === "profiles"
                  ? "管理你的独立浏览器环境，让多账号工作井然有序。"
                  : page === "groups"
                    ? "用分组整理不同业务，让每个环境各就其位。"
                    : page === "proxies"
                      ? "集中管理代理配置，为不同环境分配独立的网络。"
                      : page === "bookmarks" ? "统一编辑本机书签，与启用同步的环境保持一致。"
                      : page === "extensions"
                        ? "一份扩展代码，按窗口决定是否加载。"
                        : page === "archived"
                        ? "已移除的环境配置保留在这里，你可以随时恢复。"
                        : page === "overview"
                          ? "一览你的工作空间，从一个独立环境开始。"
                          : "打造适合你的本地工作空间。"
              }}
            </p>
          </div>
          <div class="heading-actions">
            <template v-if="page === 'profiles'"
              ><button class="button secondary" @click="chooseImport">
                <Upload :size="16" />导入环境</button
              ><button
                class="button primary"
                :disabled="!ready"
                @click="editProfile()"
              >
                <Plus :size="18" />创建窗口<span class="button-shortcut"
                  >N</span
                >
              </button></template
            ><button
              v-else-if="page === 'groups'"
              class="button primary"
              :disabled="!ready"
              @click="editGroup()"
            >
              <Plus :size="17" />创建分组</button
            ><button
              v-else-if="page === 'proxies'"
              class="button primary"
              :disabled="!ready"
              @click="editProxy()"
            >
              <Plus :size="17" />添加代理</button
            ><button
              v-else-if="page === 'overview'"
              class="button primary"
              :disabled="!ready"
              @click="editProfile()"
            >
              <Plus :size="17" />创建窗口
            </button>
          </div>
        </div>

        <div v-if="storageError" class="error-banner" role="alert">
          <Info :size="17" /><span>{{ storageError }}</span
          ><button v-if="!ready" @click="loadWorkspace">重试</button>
        </div>

        <template v-if="page === 'profiles' || page === 'archived'">
          <section class="profiles-panel">
            <div class="panel-toolbar">
              <div class="tabs" role="tablist" aria-label="环境筛选">
                <template v-if="page === 'profiles'"
                  ><button
                    :class="{ active: tab === 'all' }"
                    role="tab"
                    :aria-selected="tab === 'all'"
                    @click="tab = 'all'"
                  >
                    全部环境<span>{{ activeProfiles.length }}</span></button
                  ><button
                    :class="{ active: tab === 'favorite' }"
                    role="tab"
                    :aria-selected="tab === 'favorite'"
                    @click="tab = 'favorite'"
                  >
                    <Star :size="14" />收藏<span v-if="favoriteCount">{{
                      favoriteCount
                    }}</span>
                  </button></template
                ><button v-else class="active" role="tab" aria-selected="true">
                  已归档环境<span>{{ archivedProfiles.length }}</span>
                </button>
              </div>
              <div class="panel-right">
                <span class="kernel-label"><i></i>{{ browser.path ? `Camoufox ${browser.version ?? ""}` : "未检测到内核" }}</span
                ><button class="text-button" @click="openModal('kernel')">
                  配置内核<ChevronRight :size="13" />
                </button>
              </div>
            </div>
            <div class="filter-toolbar">
              <div class="search-box">
                <Search :size="17" /><input
                  ref="searchInput"
                  v-model="query"
                  aria-label="搜索环境"
                  placeholder="搜索名称、标签、代理 IP…"
                /><button
                  v-if="query"
                  class="icon-button"
                  aria-label="清除搜索"
                  @click="query = ''"
                >
                  <X :size="14" /></button
                ><kbd v-else>⌘ K</kbd>
              </div>
              <div class="select-wrap group-select">
                <Folder :size="15" /><select
                  v-model="groupFilter"
                  aria-label="筛选分组"
                >
                  <option value="">全部分组</option>
                  <option
                    v-for="g in workspace.groups"
                    :key="g.id"
                    :value="g.id"
                  >
                    {{ g.name }}
                  </option>
                  <option value="__none">未分组</option></select
                ><ChevronDown :size="13" />
              </div>
              <button
                class="button filter-button"
                :class="{ toggled: showFilters }"
                @click="showFilters = !showFilters"
              >
                <SlidersHorizontal :size="15" />筛选<span
                  v-if="sort !== 'serial'"
                  class="filter-active-dot"
                ></span>
              </button>
              <div class="toolbar-spacer"></div>
              <button
                class="icon-button group-toggle"
                :class="{ toggled: showGroups }"
                title="查看分组"
                aria-label="查看分组"
                @click="showGroups = !showGroups"
              >
                <FolderOpen :size="17" />
              </button>
              <div class="view-toggle">
                <button
                  :class="{ active: view === 'list' }"
                  title="列表视图"
                  aria-label="列表视图"
                  @click="view = 'list'"
                >
                  <List :size="16" /></button
                ><button
                  :class="{ active: view === 'grid' }"
                  title="卡片视图"
                  aria-label="卡片视图"
                  @click="view = 'grid'"
                >
                  <Grid2X2 :size="15" />
                </button>
              </div>
              <div class="toolbar-divider"></div>
              <div class="dropdown-anchor" data-menu>
                <button
                  class="icon-button"
                  title="更多操作"
                  aria-label="更多操作"
                  @click="menu = menu === 'toolbar' ? '' : 'toolbar'"
                >
                  <Ellipsis :size="20" />
                </button>
                <div
                  v-if="menu === 'toolbar'"
                  class="dropdown toolbar-dropdown"
                >
                  <button @click="exportBackup">
                    <Download :size="15" />导出工作空间</button
                  ><button @click="chooseImport">
                    <Upload :size="15" />从备份恢复</button
                  ><button @click="navigate('archived')">
                    <Archive :size="15" />查看回收站
                  </button>
                </div>
              </div>
            </div>
            <div v-if="showFilters" class="expanded-filter">
              <span>排序方式</span
              ><select v-model="sort" aria-label="排序方式">
                <option value="serial">序号从小到大</option>
                <option value="newest">最近创建优先</option>
                <option value="name">按环境名称</option></select
              ><button
                class="text-button"
                @click="
                  query = '';
                  groupFilter = '';
                  sort = 'serial';
                  tab = 'all';
                "
              >
                重置筛选
              </button>
            </div>
            <div v-if="showGroups" class="group-strip">
              <button
                :class="{ selected: !groupFilter }"
                @click="groupFilter = ''"
              >
                全部分组<span>{{ activeProfiles.length }}</span></button
              ><button
                v-for="g in workspace.groups"
                :key="g.id"
                :class="{ selected: groupFilter === g.id }"
                @click="groupFilter = g.id"
              >
                <i :style="{ background: g.color }"></i>{{ g.name
                }}<span>{{ groupCount(g.id) }}</span></button
              ><button class="group-strip-add" @click="editGroup()">
                <Plus :size="14" />新建分组
              </button>
            </div>
            <div v-if="selected.length" class="selection-toolbar">
              <span
                ><CheckCheck :size="16" />已选择
                <strong>{{ selected.length }}</strong> 个环境</span
              >
              <div class="selection-actions">
                <div class="dropdown-anchor" data-menu>
                  <button
                    class="text-button"
                    @click="menu = menu === 'move' ? '' : 'move'"
                  >
                    <FolderInput :size="15" />移动到分组<ChevronDown
                      :size="12"
                    />
                  </button>
                  <div v-if="menu === 'move'" class="dropdown">
                    <button @click="moveSelected('')">未分组</button
                    ><button
                      v-for="g in workspace.groups"
                      :key="g.id"
                      @click="moveSelected(g.id)"
                    >
                      <i :style="{ background: g.color }"></i>{{ g.name }}
                    </button>
                  </div>
                </div>
                <button
                  class="text-button"
                  @click="archive([...selected], page !== 'archived')"
                >
                  <component
                    :is="page === 'archived' ? RefreshCw : Archive"
                    :size="15"
                  />{{
                    page === "archived" ? "恢复环境" : "移入回收站"
                  }}</button
                ><button class="text-button" @click="selected = []">
                  取消选择
                </button>
              </div>
            </div>

            <div
              v-if="view === 'list'"
              class="table-scroll"
              @scroll="menu = ''"
            >
              <table class="profile-table">
                <thead>
                  <tr>
                    <th class="check-cell">
                      <input
                        type="checkbox"
                        aria-label="选择当前页所有环境"
                        :checked="allSelected"
                        :indeterminate="selected.length > 0 && !allSelected"
                        @change="toggleAll"
                      />
                    </th>
                    <th class="serial-cell">
                      <button @click="sort = 'serial'">
                        序号<ArrowDown
                          :size="12"
                          :class="{ 'blue-icon': sort === 'serial' }"
                        />
                      </button>
                    </th>
                    <th class="name-cell">
                      <button
                        @click="sort = sort === 'name' ? 'serial' : 'name'"
                      >
                        环境名称<ArrowUpDown :size="12" />
                      </button>
                    </th>
                    <th class="group-cell">分组</th>
                    <th class="proxy-cell">代理 IP</th>
                    <th class="tag-cell">标签</th>
                    <th class="os-cell">配置</th>
                    <th class="created-cell">
                      <button
                        @click="sort = sort === 'newest' ? 'serial' : 'newest'"
                      >
                        创建时间<ArrowDown
                          :size="12"
                          :class="{ 'blue-icon': sort === 'newest' }"
                        />
                      </button>
                    </th>
                    <th class="action-cell">操作</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="profile in rows"
                    :key="profile.id"
                    :class="{ 'row-selected': selected.includes(profile.id) }"
                  >
                    <td class="check-cell">
                      <input
                        type="checkbox"
                        :aria-label="`选择 ${profile.name}`"
                        :checked="selected.includes(profile.id)"
                        @change="toggleSelection(profile.id)"
                      />
                    </td>
                    <td class="serial-cell">
                      <span class="serial-number">{{
                        String(profile.serial).padStart(2, "0")
                      }}</span>
                    </td>
                    <td class="name-cell">
                      <div class="profile-identity">
                        <span
                          class="profile-avatar"
                          :class="`avatar-${profile.serial % 4}`"
                          ><Fingerprint :size="21" :stroke-width="1.6"
                        /></span>
                        <div>
                          <button
                            class="profile-name"
                            @click="editProfile(profile)"
                          >
                            {{ profile.name }}</button
                          ><small>{{
                            profile.notes || "独立浏览器环境"
                          }}</small>
                        </div>
                        <button
                          class="favorite-button"
                          :class="{ favorited: profile.favorite }"
                          :title="profile.favorite ? '取消收藏' : '收藏环境'"
                          :aria-label="`${profile.favorite ? '取消收藏' : '收藏'} ${profile.name}`"
                          @click="favorite(profile)"
                        >
                          <Star
                            :size="13"
                            :fill="profile.favorite ? 'currentColor' : 'none'"
                          />
                        </button>
                      </div>
                    </td>
                    <td>
                      <span
                        v-if="groupsFor(profile.groupId)"
                        class="group-badge"
                        :style="{
                          '--group-color': groupsFor(profile.groupId)!.color,
                        }"
                        ><i></i>{{ groupsFor(profile.groupId)!.name }}</span
                      ><span v-else class="muted-text">未分组</span>
                    </td>
                    <td>
                      <div v-if="proxyFor(profile.proxyId)" class="proxy-info">
                        <span class="proxy-address"
                          ><span
                            class="protocol-badge"
                            :class="{
                              http:
                                proxyFor(profile.proxyId)!.protocol === 'HTTP',
                            }"
                            >{{
                              proxyFor(profile.proxyId)!.protocol === "HTTP"
                                ? "H"
                                : "S"
                            }}</span
                          >{{ proxyFor(profile.proxyId)!.host }}</span
                        ><small
                          ><span>{{
                            countryFlag(proxyFor(profile.proxyId)!.country)
                          }}</span
                          >{{
                            countryNames[proxyFor(profile.proxyId)!.country] ||
                            proxyFor(profile.proxyId)!.country
                          }}<span class="proxy-unchecked">· {{ health(proxyFor(profile.proxyId)!) }}</span></small
                        >
                      </div>
                      <div v-else class="direct-connection">
                        <Globe2 :size="14" />本地直连
                      </div>
                    </td>
                    <td>
                      <div class="tag-list">
                        <span
                          v-for="tag in profile.tags.slice(0, 2)"
                          :key="tag"
                          class="tag"
                          :class="{
                            'tag-green': tag === '主账号',
                            'tag-amber': tag === '测试',
                            'tag-purple': ['Amazon', 'Shopify'].includes(tag),
                          }"
                          >{{ tag }}</span
                        ><span v-if="!profile.tags.length" class="muted-text"
                          >—</span
                        >
                      </div>
                    </td>
                    <td>
                      <span
                        class="os-symbol"
                        :class="{ windows: profile.os === 'Windows' }"
                        :title="profile.os"
                        ><svg
                          v-if="profile.os === 'Windows'"
                          width="16"
                          height="16"
                          viewBox="0 0 16 16"
                          fill="currentColor"
                        >
                          <path
                            d="M0 2.2 6.5 1.3v6.1H0Zm7.3-1L16 0v7.4H7.3ZM0 8.2h6.5v6.1L0 13.4Zm7.3 0H16V16l-8.7-1.3Z"
                          /></svg
                        ><Monitor v-else :size="17" /><small>{{
                          profile.os === "Windows" ? "Win" : profile.os
                        }}</small></span
                      >
                    </td>
                    <td class="created-cell">
                      <span>{{ formatDate(profile.createdAt) }}</span
                      ><small :class="['status-idle', { 'status-running': instanceFor(profile.id) }]" :title="instanceFor(profile.id) ? `PID ${instanceFor(profile.id)!.pid}` : browser.errors[profile.id]"><i></i>{{ browserLabel(profile.id) }}</small>
                    </td>
                    <td class="action-cell">
                      <div class="row-actions">
                        <button
                          v-if="page === 'archived'"
                          class="open-button restore-button"
                          @click="archive([profile.id], false)"
                        >
                          <RefreshCw :size="13" />恢复</button
                        ><button
                          v-else
                          class="open-button"
                          :class="{ 'close-browser-button': instanceFor(profile.id) }"
                          :disabled="browserBusy.includes(profile.id) || instanceFor(profile.id)?.stopping"
                          @click="toggleBrowser(profile)"
                        >
                          <component :is="instanceFor(profile.id) ? Square : Zap" :size="13" />{{ instanceFor(profile.id)?.stopping ? "关闭中" : instanceFor(profile.id) ? "关闭" : "打开" }}
                        </button>
                        <div class="dropdown-anchor row-menu-anchor" data-menu>
                          <button
                            class="icon-button row-more"
                            :aria-label="`${profile.name} 更多操作`"
                            @click="toggleProfileMenu(profile.id, $event)"
                          >
                            <Ellipsis :size="17" />
                          </button>
                          <div
                            v-if="menu === profile.id"
                            class="dropdown row-dropdown"
                            :style="rowMenuPosition"
                          >
                            <button @click="editProfile(profile)">
                              <Settings2 :size="14" />编辑环境</button
                            ><button @click="duplicateProfile(profile)">
                              <Copy :size="14" />复制配置</button
                            ><button
                              @click="
                                favorite(profile);
                                menu = '';
                              "
                            >
                              <Star :size="14" />{{
                                profile.favorite ? "取消收藏" : "收藏环境"
                              }}
                            </button>
                            <div class="dropdown-line"></div>
                            <button
                              @click="archive([profile.id], !profile.archived)"
                            >
                              <component
                                :is="profile.archived ? RefreshCw : Archive"
                                :size="14"
                              />{{
                                profile.archived ? "恢复环境" : "移入回收站"
                              }}
                            </button>
                          </div>
                        </div>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div v-else class="profile-grid">
              <article
                v-for="profile in rows"
                :key="profile.id"
                class="profile-card"
                :class="{ selected: selected.includes(profile.id) }"
              >
                <div class="profile-card-top">
                  <input
                    type="checkbox"
                    :checked="selected.includes(profile.id)"
                    :aria-label="`选择 ${profile.name}`"
                    @change="toggleSelection(profile.id)"
                  /><span class="serial-number"
                    >#{{ String(profile.serial).padStart(2, "0") }}</span
                  ><button
                    class="favorite-button"
                    :class="{ favorited: profile.favorite }"
                    aria-label="收藏环境"
                    @click="favorite(profile)"
                  >
                    <Star
                      :size="16"
                      :fill="profile.favorite ? 'currentColor' : 'none'"
                    />
                  </button>
                </div>
                <span
                  class="profile-avatar"
                  :class="`avatar-${profile.serial % 4}`"
                  ><Fingerprint :size="25" /></span
                ><button class="profile-name" @click="editProfile(profile)">
                  {{ profile.name }}</button
                ><span class="card-group"
                  ><i
                    :style="{
                      background:
                        groupsFor(profile.groupId)?.color ?? '#a2adbf',
                    }"
                  ></i
                  >{{ groupsFor(profile.groupId)?.name ?? "未分组" }}</span
                >
                <div class="card-proxy">
                  <Globe2 :size="14" />{{
                    proxyFor(profile.proxyId)?.host ?? "本地直连"
                  }}<span>{{ profile.os }}</span>
                </div>
                <div class="card-bottom">
                  <span :class="['status-idle', { 'status-running': instanceFor(profile.id) }]" :title="instanceFor(profile.id) ? `PID ${instanceFor(profile.id)!.pid}` : browser.errors[profile.id]"><i></i>{{ browserLabel(profile.id) }}</span
                  ><button
                    class="open-button"
                    :class="{ 'close-browser-button': instanceFor(profile.id) }"
                    :disabled="browserBusy.includes(profile.id) || instanceFor(profile.id)?.stopping"
                    @click="
                      page === 'archived'
                        ? archive([profile.id], false)
                        : toggleBrowser(profile)
                    "
                  >
                    <component
                      :is="page === 'archived' ? RefreshCw : instanceFor(profile.id) ? Square : Zap"
                      :size="13"
                    />{{ page === "archived" ? "恢复" : instanceFor(profile.id)?.stopping ? "关闭中" : instanceFor(profile.id) ? "关闭" : "打开" }}
                  </button>
                </div>
              </article>
            </div>
            <div v-if="!rows.length" class="empty-state">
              <div class="empty-icon">
                <component
                  :is="
                    page === 'archived'
                      ? Archive
                      : query || groupFilter
                        ? Search
                        : Fingerprint
                  "
                  :size="30"
                  :stroke-width="1.4"
                />
              </div>
              <h3>
                {{
                  query || groupFilter
                    ? "没有找到匹配的环境"
                    : page === "archived"
                      ? "回收站很干净"
                      : tab === "favorite"
                        ? "还没有收藏的环境"
                        : "创建你的第一个独立环境"
                }}
              </h3>
              <p>
                {{
                  query || groupFilter
                    ? "换个关键词，或调整分组筛选试试。"
                    : page === "archived"
                      ? "移入回收站的环境会出现在这里。"
                      : tab === "favorite"
                        ? "点击环境名称旁的星标，即可快速找到常用环境。"
                        : "从配置身份、代理和指纹参数开始。"
                }}
              </p>
              <button
                v-if="
                  !query &&
                  !groupFilter &&
                  page !== 'archived' &&
                  tab !== 'favorite'
                "
                class="button primary"
                :disabled="!ready"
                @click="editProfile()"
              >
                <Plus :size="16" />创建窗口</button
              ><button
                v-if="query || groupFilter"
                class="button secondary"
                @click="
                  query = '';
                  groupFilter = '';
                "
              >
                清除筛选
              </button>
            </div>
            <div class="table-bottom">
              <div class="table-summary">
                <span class="live-dot"></span
                >{{
                  selected.length
                    ? `已选 ${selected.length} 个环境`
                    : "所有环境独立配置"
                }}<span class="table-summary-divider">·</span
                ><span>{{ filtered.length }} 个环境</span>
              </div>
              <div class="pagination">
                <span>共 {{ filtered.length }} 条</span
                ><select v-model="pageSize" aria-label="每页条数">
                  <option :value="10">10 条 / 页</option>
                  <option :value="20">20 条 / 页</option>
                  <option :value="50">50 条 / 页</option></select
                ><button
                  class="icon-button"
                  :disabled="pageNumber <= 1"
                  aria-label="上一页"
                  @click="pageNumber--"
                >
                  <ChevronLeft :size="17" /></button
                ><span class="current-page">{{ pageNumber }}</span
                ><button
                  class="icon-button"
                  :disabled="pageNumber >= pageCount"
                  aria-label="下一页"
                  @click="pageNumber++"
                >
                  <ChevronRight :size="17" />
                </button>
              </div>
            </div>
          </section>
          <div class="page-footnote">
            <span
              ><ShieldCheck :size="13" />本地存储 · 你的工作空间，由你掌控</span
            ><span>STARPERSONA WORKSPACE</span>
          </div>
          <div v-if="page === 'profiles'" class="workspace-note">
            <div class="note-illustration">
              <span class="note-orbit"></span
              ><Fingerprint :size="36" :stroke-width="1.15" /><i></i>
            </div>
            <div>
              <span class="note-eyebrow">A SPACE FOR EVERY IDENTITY</span>
              <h3>身份独立，工作更从容。</h3>
              <p>把不同业务放进各自的环境，把专注留给重要的事。</p>
            </div>
            <button @click="openModal('help')">
              了解环境隔离<ArrowRight :size="15" />
            </button>
          </div>
        </template>

        <template v-else-if="page === 'overview'">
          <section class="overview-hero">
            <div>
              <span class="note-eyebrow">YOUR PERSONAL WORKSPACE</span>
              <h2>让每一个身份，<br />都有独立的空间<span>。</span></h2>
              <p>环境、分组、代理，在一个工作空间中井然有序。</p>
              <button class="button primary" @click="navigate('profiles')">
                进入环境管理<ArrowRight :size="16" />
              </button>
            </div>
            <div class="hero-art">
              <div class="hero-orbit one"></div>
              <div class="hero-orbit two"></div>
              <div class="hero-fingerprint">
                <Fingerprint :size="110" :stroke-width="0.8" />
              </div>
              <span class="hero-chip chip-a"
                ><ShieldCheck :size="18" />独立身份</span
              ><span class="hero-chip chip-b"
                ><Globe2 :size="18" />专属网络</span
              >
            </div>
          </section>
          <div class="stat-grid">
            <article
              v-for="stat in [
                {
                  label: '浏览器环境',
                  value: activeProfiles.length,
                  icon: Monitor,
                  text: '已保存的独立配置',
                },
                {
                  label: '业务分组',
                  value: workspace.groups.length,
                  icon: Folder,
                  text: '让不同业务各就其位',
                },
                {
                  label: '代理配置',
                  value: workspace.proxies.length,
                  icon: Globe2,
                  text: '集中管理网络入口',
                },
                {
                  label: '收藏环境',
                  value: favoriteCount,
                  icon: Star,
                  text: '快速找到常用身份',
                },
              ]"
              :key="stat.label"
              class="stat-card"
            >
              <component :is="stat.icon" :size="20" /><span>{{
                stat.label
              }}</span
              ><strong>{{ stat.value }}<small>个</small></strong>
              <p>{{ stat.text }}</p>
            </article>
          </div>
          <section class="simple-panel">
            <div class="section-heading">
              <h2>最近创建</h2>
              <button class="text-button" @click="navigate('profiles')">
                查看全部<ArrowRight :size="14" />
              </button>
            </div>
            <div
              v-for="profile in [...activeProfiles]
                .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
                .slice(0, 4)"
              :key="profile.id"
              class="recent-row"
            >
              <span
                class="profile-avatar"
                :class="`avatar-${profile.serial % 4}`"
                ><Fingerprint :size="20" /></span
              ><button class="profile-name" @click="editProfile(profile)">
                {{ profile.name }}</button
              ><span class="muted-text">{{
                groupsFor(profile.groupId)?.name ?? "未分组"
              }}</span
              ><time>{{ formatDate(profile.createdAt) }}</time
              ><button class="open-button" @click="editProfile(profile)">
                管理<ChevronRight :size="13" />
              </button>
            </div>
            <p v-if="!activeProfiles.length" class="muted-text">
              还没有环境，点击右上角创建。
            </p>
          </section>
        </template>

        <template v-else-if="page === 'groups'">
          <div class="entity-grid">
            <article
              v-for="g in workspace.groups"
              :key="g.id"
              class="group-card"
            >
              <div class="group-card-header">
                <span class="folder-art" :style="{ '--group-color': g.color }"
                  ><Folder :size="27" :stroke-width="1.5"
                /></span>
                <div class="dropdown-anchor" data-menu>
                  <button
                    class="icon-button"
                    :aria-label="`${g.name} 更多操作`"
                    @click="menu = menu === g.id ? '' : g.id"
                  >
                    <Ellipsis :size="19" />
                  </button>
                  <div v-if="menu === g.id" class="dropdown">
                    <button @click="editGroup(g)">
                      <Settings2 :size="14" />编辑分组</button
                    ><button
                      @click="
                        editId = g.id;
                        openModal('deleteGroup');
                      "
                    >
                      <Trash2 :size="14" />删除分组
                    </button>
                  </div>
                </div>
              </div>
              <h2>{{ g.name }}</h2>
              <p>{{ groupCount(g.id) }} 个浏览器环境</p>
              <div class="group-card-bottom">
                <div class="mini-avatars">
                  <span
                    v-for="p in activeProfiles
                      .filter((p) => p.groupId === g.id)
                      .slice(0, 3)"
                    :key="p.id"
                    :class="`avatar-${p.serial % 4}`"
                    ><Fingerprint :size="14" /></span
                  ><small v-if="groupCount(g.id) > 3"
                    >+{{ groupCount(g.id) - 3 }}</small
                  >
                </div>
                <button
                  class="text-button"
                  @click="
                    navigate('profiles');
                    groupFilter = g.id;
                  "
                >
                  查看环境<ArrowRight :size="14" />
                </button>
              </div>
            </article>
            <button
              class="add-entity-card"
              :disabled="!ready"
              @click="editGroup()"
            >
              <span><Plus :size="24" /></span><strong>创建新分组</strong
              ><small>给不同业务一个专属空间</small>
            </button>
          </div>
          <div class="info-note">
            <Info :size="16" />删除分组不会删除环境，环境将自动归入「未分组」。
          </div>
        </template>

        <template v-else-if="page === 'proxies'">
          <ProxyNetwork ref="proxyNet" />
          <section class="simple-panel proxies-panel">
            <div class="section-heading proxy-heading">
              <h2>
                代理配置<span class="small-count">{{
                  workspace.proxies.length
                }}</span>
              </h2>
              <div class="proxy-filters">
                <div class="search-box proxy-search">
                  <Search :size="16" /><input
                    v-model="proxyQuery"
                    aria-label="搜索代理"
                    placeholder="搜索名称、主机、端口或地区…"
                  /><button
                    v-if="proxyQuery"
                    class="icon-button"
                    aria-label="清除搜索"
                    @click="proxyQuery = ''"
                  >
                    <X :size="14" />
                  </button>
                </div>
                <div class="select-wrap proxy-select">
                  <select v-model="proxyProtocol" aria-label="按协议筛选">
                    <option value="">全部协议</option>
                    <option value="SOCKS5">SOCKS5</option>
                    <option value="HTTP">HTTP</option></select
                  ><ChevronDown :size="13" />
                </div>
                <div class="select-wrap proxy-select">
                  <select v-model="proxyStatus" aria-label="按状态筛选">
                    <option value="">全部状态</option>
                    <option value="ok">可用</option>
                    <option value="fail">失败</option>
                    <option value="unknown">未检测</option></select
                  ><ChevronDown :size="13" />
                </div>
                <button
                  v-if="proxyFiltered"
                  class="text-button proxy-filter-clear"
                  @click="resetProxyFilters"
                >
                  清除筛选
                </button>
              </div>
              <span class="muted-text">{{
                proxyFiltered
                  ? `筛选后 ${proxyRows.length} / ${workspace.proxies.length} 条`
                  : "支持 SOCKS5 / HTTP · 无认证代理"
              }}</span>
            </div>
            <div v-if="proxyRows.length" class="table-scroll">
              <table class="proxy-table">
                <thead>
                  <tr>
                    <th>代理名称</th>
                    <th>协议</th>
                    <th>主机地址</th>
                    <th>地区</th>
                    <th>关联环境</th>
                    <th>状态</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="proxy in proxyRows" :key="proxy.id">
                    <td>
                      <span class="proxy-name"
                        ><span class="proxy-icon"><Globe2 :size="18" /></span
                        >{{ proxy.name }}</span
                      >
                    </td>
                    <td>
                      <span class="tag">{{ proxy.protocol }}</span>
                    </td>
                    <td class="mono-text">{{ proxy.host }}:{{ proxy.port }}</td>
                    <td>
                      {{ countryFlag(proxy.country) }}
                      {{ countryNames[proxy.country] || proxy.country || "未知" }}
                    </td>
                    <td>
                      {{
                        activeProfiles.filter((p) => p.proxyId === proxy.id)
                          .length
                      }}
                      个环境
                    </td>
                    <td>
                      <span class="proxy-health" :class="{good:proxy.check?.ok,bad:proxy.check&&!proxy.check.ok}" :title="proxy.check?.at">{{ health(proxy) }}</span>
                      <small v-if="proxy.check?.bytesPerSecond!=null" class="proxy-speed" :title="`上次成功测速：${proxy.check.speedAt}`">{{ (proxy.check.bytesPerSecond*8/1e6).toFixed(2) }} Mbps</small>
                    </td>
                    <td>
                      <div class="row-actions">
                        <button class="text-button" :disabled="!desktop||!ready||proxyNet?.busy" @click="proxyNet?.test(proxy,false)">测连通</button>
                        <button class="text-button" title="下载 1 MiB，含建连时间" :disabled="!desktop||!ready||proxyNet?.busy" @click="proxyNet?.test(proxy,true)">测速</button>
                        <button class="text-button" :disabled="proxyNet?.busy" @click="editProxy(proxy)">
                          编辑</button
                        ><button
                          class="icon-button"
                          :aria-label="`删除代理 ${proxy.name}`"
                          @click="
                            editId = proxy.id;
                            openModal('deleteProxy');
                          "
                        >
                          <Trash2 :size="15" />
                        </button>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
            <div v-if="!proxyRows.length" class="empty-state">
              <div class="empty-icon"><Globe2 :size="30" /></div>
              <template v-if="workspace.proxies.length">
                <h3>没有符合条件的代理</h3>
                <p>换个关键词，或清除协议和状态筛选后再看。</p>
                <button class="button secondary" @click="resetProxyFilters">
                  清除筛选
                </button>
              </template>
              <template v-else>
                <h3>添加你的第一个代理</h3>
                <p>
                  保存网络配置后，可在创建环境时选择。也可以从 StarPort
                  导入本机代理端口。
                </p>
                <button class="button primary" @click="editProxy()">
                  <Plus :size="16" />添加代理
                </button>
              </template>
            </div>
          </section>
        </template>

        <Bookmarks v-else-if="page === 'bookmarks'" />
        <ExtensionCenter
          v-else-if="page === 'extensions'"
          :profiles="activeProfiles.map((profile) => ({ id: profile.id, name: profile.name, serial: profile.serial }))"
          :running="browser.instances.length > 0"
          @notify="(message, kind) => notify(message, kind)"
        />

        <template v-else-if="page === 'settings'">
          <div class="settings-layout">
            <form
              class="simple-panel settings-panel"
              @submit.prevent="saveSettings"
            >
              <div class="section-heading">
                <h2>工作空间偏好</h2>
                <Settings2 :size="18" />
              </div>
              <label class="form-field"
                >工作空间名称<input
                  v-model="settingsDraft.name"
                  maxlength="80"
                  required
                  placeholder="我的工作空间"
                /><small>显示在左侧工作空间入口。</small></label
              >
              <div class="form-row">
                <label class="form-field"
                  >默认操作系统<select v-model="settingsDraft.defaultOs">
                    <option>Windows</option>
                    <option>macOS</option>
                    <option>Linux</option>
                  </select></label
                ><label class="form-field"
                  >默认时区<select v-model="settingsDraft.defaultTimezone">
                    <option>Asia/Shanghai</option>
                    <option>America/Los_Angeles</option>
                    <option>America/New_York</option>
                    <option>Asia/Tokyo</option>
                    <option>Asia/Singapore</option>
                    <option>Europe/London</option>
                    <option>UTC</option>
                  </select></label
                >
              </div>
              <div class="settings-form-footer">
                <span>仅影响之后创建的环境</span
                ><button
                  class="button primary"
                  :disabled="saving || !ready"
                  type="submit"
                >
                  <Check :size="16" />保存设置
                </button>
              </div>
            </form>
            <section class="simple-panel settings-panel">
              <div class="section-heading">
                <h2>数据与备份</h2>
                <Database :size="18" />
              </div>
              <div class="settings-info-row">
                <span class="settings-info-icon"><HardDrive :size="20" /></span>
                <div>
                  <strong>{{
                    desktop ? "本地文件存储" : "浏览器预览模式"
                  }}</strong>
                  <p>
                    {{
                      desktop
                        ? "配置由 Rust 保存到系统应用数据目录。"
                        : "预览配置保存在当前浏览器，桌面版使用独立本地文件。"
                    }}
                  </p>
                </div>
                <span class="tag tag-green">{{
                  ready ? "已就绪" : "读取中"
                }}</span>
              </div>
              <div class="backup-actions">
                <button
                  class="button secondary"
                  :disabled="!ready"
                  @click="exportBackup"
                >
                  <ArrowDownToLine :size="16" />导出备份</button
                ><button class="button secondary" @click="chooseImport">
                  <Upload :size="16" />恢复备份
                </button>
              </div>
              <p class="settings-hint">
                备份包含环境、分组和代理地址，请妥善保管。恢复前可先导出当前数据。
              </p>
            </section>
            <section class="simple-panel settings-panel kernel-settings">
              <div class="section-heading">
                <h2>浏览器内核</h2>
                <Fingerprint :size="18" />
              </div>
              <div class="settings-info-row">
                <span class="kernel-art"
                  ><Fingerprint :size="30" :stroke-width="1.5"
                /></span>
                <div>
                  <strong>Camoufox</strong>
                  <p>基于 Firefox 的指纹浏览器内核</p>
                </div>
                <span class="neutral-status"><i></i>{{ browser.path ? "已检测" : "未检测到" }}</span>
              </div>
              <p class="settings-hint">
                {{ browser.path ? `Camoufox ${browser.version ?? ""} · ${browser.instances.length} 个环境运行中` : "选择本机已下载的 Camoufox，每个环境使用独立数据目录。" }}
              </p>
              <button class="text-button" @click="openModal('kernel')">
                配置内核<ArrowRight :size="14" />
              </button>
            </section>
            <section class="about-card">
              <span class="brand-mark"><Sparkles :size="26" /></span>
              <h3>星相 StarPersona</h3>
              <p>独立的身份，有序的工作空间。</p>
              <button v-if="desktop" class="button secondary" :disabled="updating||!!browser.instances.length" @click="upgrade">{{updating?'Homebrew 更新中…':'通过 Homebrew 检查并更新'}}</button>
              <span>v0.2.0 Beta · Tauri 2 + Rust + Vue 3</span
              ><small>开源 · 本地优先 · MIT License</small>
            </section>
          </div>
        </template>
      </main>
    </div>

    <Transition name="toast"
      ><div v-if="toast" class="toast-message" role="status">
        <span :class="['toast-icon', { info: toastKind === 'info' }]"
          ><component
            :is="toastKind === 'success' ? Check : Info"
            :size="15" /></span
        >{{ toast
        }}<button class="icon-button" aria-label="关闭提示" @click="toast = ''">
          <X :size="14" />
        </button></div
    ></Transition>
    <input
      ref="importInput"
      type="file"
      accept=".json,application/json"
      class="visually-hidden"
      aria-label="导入工作空间备份"
      @change="importFile"
    />

    <Transition name="modal"
      ><div
        v-if="modal"
        class="modal-overlay"
        :class="{ 'drawer-overlay': modal === 'profile' }"
        @mousedown.self="closeModal"
      >
        <section
          v-if="modal === 'profile' && draft"
          class="profile-drawer"
          role="dialog"
          aria-modal="true"
          aria-labelledby="profile-title"
        >
          <header class="drawer-header">
            <div>
              <span class="drawer-eyebrow">BROWSER ENVIRONMENT</span>
              <h2 id="profile-title">
                {{ editId ? "编辑环境" : "创建浏览器环境" }}
              </h2>
              <p>给新的身份，一个独立的工作空间。</p>
            </div>
            <button class="close-button" aria-label="关闭" @click="closeModal">
              <X :size="19" />
            </button>
          </header>
          <div class="drawer-tabs">
            <button
              :class="{ active: profileTab === 'basic' }"
              @click="profileTab = 'basic'"
            >
              <Monitor :size="16" />基本配置</button><button :class="{active: profileTab === 'account'}" @click="profileTab = 'account'">账号与启动</button
            ><button
              :class="{ active: profileTab === 'fingerprint' }"
              @click="profileTab = 'fingerprint'"
            >
              <Fingerprint :size="16" />指纹配置
            </button>
          </div>
          <form class="drawer-form" @submit.prevent="saveProfile">
            <div class="drawer-content">
              <template v-if="profileTab === 'basic'"
                ><div class="form-section-title"><span>01</span>环境信息</div>
                <label class="form-field"
                  >环境名称 <span class="required">*</span
                  ><input
                    id="profile-name"
                    v-model="draft.name"
                    maxlength="80"
                    placeholder="例如：star-01-emily"
                    required
                /></label>
                <div class="form-row">
                  <label class="form-field"
                    >所属分组<select v-model="draft.groupId">
                      <option value="">未分组</option>
                      <option
                        v-for="g in workspace.groups"
                        :key="g.id"
                        :value="g.id"
                      >
                        {{ g.name }}
                      </option>
                    </select></label
                  ><label class="form-field"
                    >操作系统<select v-model="draft.os">
                      <option>Windows</option>
                      <option>macOS</option>
                      <option>Linux</option>
                    </select></label
                  >
                </div>
                <label class="form-field"
                  >标签<input
                    v-model="tagsInput"
                    maxlength="370"
                    placeholder="用逗号分隔，例如：Web3, 主账号"
                  /><small>最多 12 个标签，便于快速搜索和整理。</small></label
                ><label class="form-field"
                  >备注<textarea
                    v-model="draft.notes"
                    rows="3"
                    maxlength="1000"
                    placeholder="记录这个环境的用途或注意事项…"
                  ></textarea>
                </label>
                <div class="form-section-title"><span>02</span>网络代理</div>
                <label class="form-field"
                  >代理配置<select v-model="draft.proxyId">
                    <option value="">本地直连 · 使用当前设备网络</option>
                    <option
                      v-for="p in workspace.proxies"
                      :key="p.id"
                      :value="p.id"
                    >
                      {{ p.name }} · {{ p.host }}:{{ p.port }}
                    </option>
                  </select></label
                >
                <div class="field-note">
                  <ShieldCheck :size="17" />
                  <p>
                    每个环境启动时应用自己的代理。未选择时使用本机网络直连。
                  </p>
                </div></template>
              <template v-else-if="profileTab === 'account'">
                <div class="form-section-title"><span>01</span>账号标记</div>
                <div class="form-row"><label class="form-field">平台<input v-model="draft.platform" maxlength="100" placeholder="例如：公司门户" /></label>
                <label class="form-field">账号备注<input v-model="draft.accountLabel" maxlength="100" placeholder="例如：设计组，只填别名" /></label></div>
                <p class="settings-hint">仅供本地辨认，不会自动登录。不要填写密码或 2FA 密钥。</p>
                <label><input v-model="draft.checkDuplicate" type="checkbox" /> 保存时检查重复账号</label>
                <label><input v-model="draft.exclusiveAccount" type="checkbox" /> 同一平台和账号只允许打开一个环境</label>
                <div class="form-section-title"><span>02</span>启动页</div>
                <label class="form-field">打开指定网址<textarea v-model="startPagesInput" rows="5" placeholder="https://example.org/" /><small>每行一个 HTTP(S) 网址，最多 8 个。留空使用星相启动页。请勿填写带密码或访问令牌的链接。</small></label>
              </template><template v-else
                ><div class="fingerprint-banner">
                  <span><Fingerprint :size="28" :stroke-width="1.3" /></span>
                  <div>
                    <strong>独立身份，从指纹开始</strong>
                    <p>基础参数在下次启动时应用到 Camoufox。</p>
                  </div>
                </div>
                <label class="form-field"
                  >指纹标识
                  <div class="input-with-button">
                    <input v-model="draft.fingerprint" readonly /><button
                      type="button"
                      title="重新生成指纹标识"
                      aria-label="重新生成指纹标识"
                      @click="
                        draft.fingerprint = uid()
                          .replaceAll('-', '')
                          .slice(0, 16)
                      "
                    >
                      <RefreshCw :size="16" />
                    </button>
                  </div>
                  <small
                    >每个环境使用独立标识；此标识本身不会修改浏览器指纹。</small
                  ></label
                >
                <div class="form-row">
                  <label class="form-field"
                    >浏览器语言<select v-model="draft.language">
                      <option value="zh-CN">简体中文 · zh-CN</option>
                      <option value="en-US">English · en-US</option>
                      <option value="ja-JP">日本語 · ja-JP</option>
                      <option value="en-GB">English · en-GB</option>
                    </select></label
                  ><label class="form-field"
                    >屏幕分辨率<select v-model="draft.resolution">
                      <option>1920 × 1080</option>
                      <option>1440 × 900</option>
                      <option>2560 × 1440</option>
                      <option>1366 × 768</option>
                    </select></label
                  >
                </div>
                <label class="form-field"
                  >时区<select v-model="draft.timezone">
                    <option>Asia/Shanghai</option>
                    <option>America/Los_Angeles</option>
                    <option>America/New_York</option>
                    <option>Asia/Tokyo</option>
                    <option>Asia/Singapore</option>
                    <option>Europe/London</option>
                    <option>UTC</option>
                  </select></label
                >
                <div class="fingerprint-capabilities">
                  <span><ShieldCheck :size="16" />独立配置</span
                  ><span><Database :size="16" />本地保存</span
                  ><span><Fingerprint :size="16" />Camoufox 基础接入</span>
                </div></template
              >
              <p v-if="formError" class="form-error" role="alert">
                {{ formError }}
              </p>
            </div>
            <footer class="drawer-footer">
              <span><ShieldCheck :size="14" />配置仅保存在本机</span
              ><button
                type="button"
                class="button secondary"
                @click="closeModal"
              >
                取消</button
              ><button
                type="submit"
                class="button primary"
                :disabled="saving || !ready"
              >
                <LoaderCircle v-if="saving" class="spin" :size="16" /><Check
                  v-else
                  :size="16"
                />{{ editId ? "保存配置" : "创建环境" }}
              </button>
            </footer>
          </form>
        </section>

        <section
          v-else
          class="dialog"
          :class="{ 'help-dialog': modal === 'help' }"
          role="dialog"
          aria-modal="true"
          aria-labelledby="dialog-title"
        >
          <button
            class="close-button dialog-close"
            aria-label="关闭"
            @click="closeModal"
          >
            <X :size="19" />
          </button>
          <template v-if="modal === 'group'"
            ><span class="dialog-symbol"><Folder :size="24" /></span>
            <h2 id="dialog-title">{{ editId ? "编辑分组" : "创建新分组" }}</h2>
            <p class="dialog-description">
              把相关环境放在一起，让工作更有条理。
            </p>
            <form @submit.prevent="saveGroup">
              <label class="form-field"
                >分组名称<input
                  v-model="groupDraft.name"
                  maxlength="80"
                  placeholder="例如：跨境电商"
                  required
                  autofocus /></label
              ><label class="form-field">分组颜色</label>
              <div class="color-picker">
                <button
                  v-for="color in colors"
                  :key="color"
                  type="button"
                  :style="{ background: color }"
                  :aria-label="`选择颜色 ${color}`"
                  :class="{ selected: groupDraft.color === color }"
                  @click="groupDraft.color = color"
                >
                  <Check v-if="groupDraft.color === color" :size="18" />
                </button>
              </div>
              <p v-if="formError" class="form-error" role="alert">
                {{ formError }}
              </p>
              <footer class="dialog-footer">
                <button
                  type="button"
                  class="button secondary"
                  @click="closeModal"
                >
                  取消</button
                ><button
                  class="button primary"
                  :disabled="saving || !ready"
                  type="submit"
                >
                  {{ editId ? "保存分组" : "创建分组" }}
                </button>
              </footer>
            </form></template
          >
          <template v-else-if="modal === 'proxy'"
            ><span class="dialog-symbol"><Globe2 :size="24" /></span>
            <h2 id="dialog-title">
              {{ editId ? "编辑代理" : "添加代理配置" }}
            </h2>
            <p class="dialog-description">支持无认证的 SOCKS5 与 HTTP 代理。</p>
            <form @submit.prevent="saveProxy">
              <label class="form-field"
                >代理名称<input
                  v-model="proxyDraft.name"
                  maxlength="80"
                  placeholder="例如：美国 · 洛杉矶"
                  required
              /></label>
              <div class="form-row">
                <label class="form-field"
                  >代理协议<select v-model="proxyDraft.protocol">
                    <option>SOCKS5</option>
                    <option>HTTP</option>
                  </select></label
                ><label class="form-field"
                  >国家 / 地区<select v-model="proxyDraft.country">
                    <option value="">未知</option>
                    <option
                      v-for="(name, code) in countryNames"
                      :key="code"
                      :value="code"
                    >
                      {{ name }}
                    </option>
                  </select></label
                >
              </div>
              <div class="form-row proxy-address-fields">
                <label class="form-field"
                  >主机地址<input
                    v-model="proxyDraft.host"
                    maxlength="253"
                    placeholder="IP 或主机名"
                    required /></label
                ><label class="form-field"
                  >端口<input
                    v-model.number="proxyDraft.port"
                    type="number"
                    min="1"
                    max="65535"
                    required
                /></label>
              </div>
              <div class="field-note">
                <Info :size="16" />
                <p>
                  保存不会联网。回到代理列表可单独测连通或测速，环境下次启动时应用代理。
                </p>
              </div>
              <p v-if="formError" class="form-error" role="alert">
                {{ formError }}
              </p>
              <footer class="dialog-footer">
                <button
                  type="button"
                  class="button secondary"
                  @click="closeModal"
                >
                  取消</button
                ><button
                  class="button primary"
                  :disabled="saving || !ready"
                  type="submit"
                >
                  保存代理
                </button>
              </footer>
            </form></template
          >
          <template v-else-if="modal === 'kernel'">
            <span class="dialog-symbol kernel-symbol"><Fingerprint :size="29" :stroke-width="1.4" /></span>
            <h2 id="dialog-title">本地浏览器内核</h2>
            <p class="dialog-description">复用一份 Camoufox，各环境独立保存数据。</p>
            <template v-if="desktop">
              <label class="form-field kernel-path-field">Camoufox 应用路径
                <div class="input-with-button"><input v-model="kernelDraft.executable" placeholder="留空自动检测，或输入 Camoufox.app 路径" /><button type="button" aria-label="选择 Camoufox" @click="chooseKernel"><FolderOpen :size="17" /></button></div>
                <small v-if="browser.path">已检测：{{ browser.path }} · {{ browser.version }}</small>
              </label>
              <KernelVersions @pick="path => { kernelDraft.executable=path }" />
              <p class="kernel-explanation">扩展改在「扩展中心」管理：代码只存一份，可以加载到所有窗口，或只指定部分窗口。这里只保存 Camoufox 的位置。</p>
              <p class="kernel-explanation">已接入操作系统标识、语言、时区、屏幕尺寸和代理。支持在代理页手动检测连通性，完整设备指纹尚未验收。</p>
              <p v-if="formError" class="form-error" role="alert">{{ formError }}</p>
              <footer class="dialog-footer"><button class="button secondary" @click="closeModal">取消</button><button class="button primary" @click="saveKernel">保存设置</button></footer>
            </template>
            <template v-else><p class="kernel-explanation">网页预览不能启动本机浏览器。请打开星相桌面应用，选择已下载的 Camoufox。</p><footer class="dialog-footer"><button class="button primary" @click="closeModal">知道了</button></footer></template>
          </template>
          <template v-else-if="modal === 'help'"
            ><span class="dialog-symbol"><Sparkles :size="24" /></span>
            <h2 id="dialog-title">你好，欢迎来到星相。</h2>
            <p class="dialog-description">独立的身份，有序的工作空间。</p>
            <div class="help-steps">
              <article>
                <span>01</span>
                <div>
                  <h3>创建一个浏览器环境</h3>
                  <p>
                    点击「创建窗口」，设置名称、分组、代理和指纹参数。配置会自动保存在本机。
                  </p>
                </div>
              </article>
              <article>
                <span>02</span>
                <div>
                  <h3>按自己的方式整理</h3>
                  <p>
                    用分组区分业务，用标签标记用途；支持收藏、搜索、批量移动和回收站恢复。
                  </p>
                </div>
              </article>
              <article>
                <span>03</span>
                <div>
                  <h3>为工作空间保留一份备份</h3>
                  <p>
                    在系统设置或列表更多菜单中导出 JSON
                    备份。恢复备份会替换当前配置，请先导出已有数据。
                  </p>
                </div>
              </article>
            </div>
            <div class="help-notice">
              <Info :size="17" />
              <p>
                已支持本地 Camoufox 启动、关闭与独立数据目录。扩展在「扩展中心」添加，代理可在「代理 IP」页检测连通和测速。基础参数已接入，完整设备指纹尚未验收。
              </p>
            </div>
            <footer class="dialog-footer">
              <span class="help-version">StarPersona v0.1.0 · MIT</span
              ><button class="button primary" @click="closeModal">
                开始使用<ArrowRight :size="15" />
              </button></footer
          ></template>
          <template v-else-if="modal === 'restore'"
            ><span class="dialog-symbol"><Upload :size="24" /></span>
            <h2 id="dialog-title">恢复工作空间备份</h2>
            <p class="dialog-description">
              这将替换当前的环境、分组、代理和设置。
            </p>
            <div class="restore-summary">
              <span
                ><strong>{{ restoreDraft?.profiles.length }}</strong>
                个环境</span
              ><span
                ><strong>{{ restoreDraft?.groups.length }}</strong> 个分组</span
              ><span
                ><strong>{{ restoreDraft?.proxies.length }}</strong>
                个代理</span
              >
            </div>
            <p class="kernel-explanation">
              请确认已导出当前工作空间，替换后无法通过回收站恢复原有配置。
            </p>
            <footer class="dialog-footer">
              <button class="button secondary" @click="exportBackup">
                先导出当前数据</button
              ><button
                class="button primary"
                :disabled="saving || !ready"
                @click="restoreBackup"
              >
                确认恢复
              </button>
            </footer></template
          >
          <template v-else
            ><span class="dialog-symbol warning-symbol"
              ><Trash2 :size="24"
            /></span>
            <h2 id="dialog-title">
              {{
                modal === "deleteGroup" ? "删除这个分组？" : "删除这个代理？"
              }}
            </h2>
            <p class="dialog-description">
              {{
                modal === "deleteGroup"
                  ? "环境配置会保留，原有环境将移动到「未分组」。"
                  : "关联环境的代理配置将切换为本地直连。此操作不会删除环境。"
              }}
            </p>
            <footer class="dialog-footer">
              <button class="button secondary" @click="closeModal">取消</button
              ><button
                class="button danger"
                :disabled="saving || !ready"
                @click="deleteEntity"
              >
                确认删除
              </button>
            </footer></template
          >
        </section>
      </div></Transition
    >
  </div>
</template>
