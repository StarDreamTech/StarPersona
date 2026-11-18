<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  FolderOpen,
  MoreHorizontal,
  Package,
  Plus,
  Puzzle,
  RefreshCw,
  Search,
  Trash2,
  X,
} from "lucide-vue-next";
import {
  classifyExtensionUrl,
  extensionBroken,
  extensions,
  filterExtensions,
  mutateExtensions,
  refreshExtensions,
  searchStore,
  type StoreAddon,
  type StorePage,
} from "./extensions";
import { desktop } from "./workspace";

const props = defineProps<{
  profiles: { id: string; name: string; serial: number }[];
  running: boolean;
}>();
const emit = defineEmits<{ notify: [message: string, kind?: "success" | "info"] }>();

const view = ref<"mine" | "broken" | "store">("mine");
const query = ref("");
const status = ref("");
const menu = ref("");
const busy = ref(false);
const error = ref("");
const urlOpen = ref(false);
const url = ref("");
const storeQuery = ref("");
const store = ref<StorePage | null>(null);
const storeBusy = ref(false);
const storeError = ref("");
let storeTimer = 0;
const assignId = ref("");
const selected = ref<string[]>([]);
const removeId = ref("");
let unlisten: (() => void) | undefined;

const rows = computed(() =>
  filterExtensions(extensions.value, view.value === "broken" ? "broken" : "mine", query.value, status.value),
);
const mineCount = computed(() => extensions.value.filter((item) => !extensionBroken(item)).length);
const brokenCount = computed(() => extensions.value.length - mineCount.value);
const assigning = computed(() => extensions.value.find((item) => item.id === assignId.value));
const removing = computed(() => extensions.value.find((item) => item.id === removeId.value));

onMounted(async () => {
  if (!desktop) return;
  try {
    await refreshExtensions();
  } catch (cause) {
    error.value = String(cause);
  }
  const { listen } = await import("@tauri-apps/api/event");
  unlisten = await listen("extensions-changed", () => {
    void refreshExtensions().catch((cause) => {
      error.value = String(cause);
    });
  });
});
onBeforeUnmount(() => {
  unlisten?.();
  window.clearTimeout(storeTimer);
});

watch(view, (next) => {
  if (next === "store" && !store.value) void loadStore(1);
});
watch(storeQuery, () => {
  if (view.value !== "store") return;
  window.clearTimeout(storeTimer);
  storeTimer = window.setTimeout(() => void loadStore(1), 300);
});

async function loadStore(page: number) {
  storeBusy.value = true;
  storeError.value = "";
  try {
    store.value = await searchStore(storeQuery.value.trim(), page);
  } catch (cause) {
    storeError.value = String(cause);
  } finally {
    storeBusy.value = false;
  }
}

function usersLabel(count: number) {
  if (count >= 10000) {
    const wan = count / 10000;
    const text = wan >= 100 ? String(Math.round(wan)) : wan.toFixed(1).replace(/\.0$/, "");
    return `${text} 万每日用户`;
  }
  return `${count} 每日用户`;
}

function added(item: StoreAddon) {
  return extensions.value.some(
    (entry) => entry.id === item.guid || entry.sourceUrl.includes(`/addon/${item.slug}`),
  );
}

async function addStore(item: StoreAddon) {
  await run("extension_import_url", { url: item.pageUrl }, "已加入扩展中心，默认不加载");
}

async function run(command: string, args: Record<string, unknown>, done: string) {
  busy.value = true;
  error.value = "";
  try {
    await mutateExtensions(command, args);
    emit("notify", done);
    return true;
  } catch (cause) {
    error.value = String(cause);
    return false;
  } finally {
    busy.value = false;
    menu.value = "";
  }
}

async function pick(directory: boolean) {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const path = await open({
    title: directory ? "选择已解压的扩展目录" : "选择 xpi 或 zip 安装包",
    directory,
    multiple: false,
    filters: directory ? undefined : [{ name: "扩展安装包", extensions: ["xpi", "zip"] }],
  });
  if (!path || Array.isArray(path)) return;
  const before = extensions.value.length;
  const ok = await run("extension_import_path", { path }, "扩展已加入，默认不加载");
  const added = extensions.value[extensions.value.length - 1];
  if (ok && extensions.value.length > before && added && extensionBroken(added)) view.value = "broken";
}

async function importUrl() {
  const kind = classifyExtensionUrl(url.value);
  if (kind === "chrome") {
    error.value = "这是 Chrome 商店地址。星相当前内核是 Camoufox（Firefox），不能加载 Chrome 扩展。";
    return;
  }
  if (kind !== "amo") {
    error.value = "请粘贴 addons.mozilla.org 的扩展详情页。";
    return;
  }
  const ok = await run("extension_import_url", { url: url.value.trim() }, "已从 Firefox 附加组件下载，默认不加载");
  if (ok) {
    urlOpen.value = false;
    url.value = "";
    view.value = "mine";
  }
}

function openAssign(id: string) {
  const item = extensions.value.find((entry) => entry.id === id);
  if (!item) return;
  assignId.value = id;
  selected.value = [...item.profileIds];
  menu.value = "";
}

async function saveAssign(scope: "all" | "profiles" | "off") {
  if (!assignId.value) return;
  const ids = scope === "profiles" ? selected.value : [];
  const ok = await run(
    "extension_set_scope",
    { id: assignId.value, scope, profileIds: ids },
    scope === "all" ? "已设为所有窗口加载" : scope === "off" ? "已停止加载" : `已指定 ${ids.length} 个窗口`,
  );
  if (ok) assignId.value = "";
}

async function toggle(item: { id: string; scope: string }) {
  await run(
    "extension_set_scope",
    { id: item.id, scope: item.scope === "all" ? "off" : "all", profileIds: [] },
    item.scope === "all" ? "已停止对所有窗口加载" : "已对所有窗口加载，重新打开窗口后生效",
  );
}

async function confirmRemove() {
  if (!removeId.value) return;
  const ok = await run("extension_remove", { id: removeId.value }, "已从扩展中心移除");
  if (ok) removeId.value = "";
}

function sourceLabel(source: string) {
  if (source === "amo") return "Firefox 附加组件";
  if (source === "xpi") return "本地安装包";
  return "本地目录";
}
</script>

<template>
  <section class="ext-center">
    <p v-if="!desktop" class="ext-note">扩展中心需要在星相桌面应用里使用。网页预览不能读取本机扩展目录。</p>
    <template v-else>
      <div class="ext-toolbar">
        <div class="ext-tabs" role="tablist">
          <button :class="{ active: view === 'mine' }" @click="view = 'mine'">我的扩展 <span>{{ mineCount }}</span></button>
          <button :class="{ active: view === 'store' }" @click="view = 'store'">Firefox 商店</button>
          <button :class="{ active: view === 'broken' }" @click="view = 'broken'">不可用 <span>{{ brokenCount }}</span></button>
        </div>
        <label v-if="view === 'store'" class="ext-search">
          <Search :size="15" />
          <input v-model="storeQuery" placeholder="搜索 Firefox 附加组件" />
        </label>
        <label v-else class="ext-search">
          <Search :size="15" />
          <input v-model="query" placeholder="搜索名称、说明或扩展 ID" />
        </label>
        <select v-if="view === 'mine'" v-model="status" aria-label="加载状态">
          <option value="">全部状态</option>
          <option value="all">所有窗口</option>
          <option value="profiles">指定窗口</option>
          <option value="off">未启用</option>
        </select>
        <div class="ext-add">
          <button class="button primary" :disabled="busy" @click="menu = menu === 'add' ? '' : 'add'">
            <Plus :size="16" />添加扩展
          </button>
          <div v-if="menu === 'add'" class="ext-menu">
            <button @click="pick(true)"><FolderOpen :size="15" />本地目录</button>
            <button @click="pick(false)"><Package :size="15" />xpi / zip 安装包</button>
            <button @click="view = 'store'; menu = ''"><Puzzle :size="15" />Firefox 商店</button>
          </div>
        </div>
      </div>
      <p class="ext-note">
        扩展代码只保存一份。打开开关后，所有窗口在下次启动时加载；也可以只指定部分窗口。每个窗口的插件设置和网站数据仍然分开。
        <template v-if="running">已打开的窗口需要重新打开后才会应用。</template>
      </p>
      <p v-if="error && view !== 'store'" class="form-error" role="alert">{{ error }}</p>
      <div v-if="view === 'store'" class="ext-store">
        <p class="ext-note">
          <template v-if="storeQuery.trim() && store">找到 {{ store.total }} 个。</template>
          <template v-else>这里是 Firefox 附加组件商店，按每日使用人数排列。</template>
          添加后默认不加载，代码只保存一份。
          <button class="text-button" @click="urlOpen = true">粘贴详情页</button>
        </p>
        <p v-if="storeError" class="form-error" role="alert">{{ storeError }}</p>
        <p v-else-if="storeBusy && !store" class="ext-note">正在读取商店…</p>
        <div v-else-if="store?.addons.length" class="ext-grid">
          <article v-for="item in store.addons" :key="item.slug" class="ext-card">
            <img v-if="item.icon" :src="item.icon" alt="" />
            <span v-else class="ext-fallback"><Puzzle :size="22" /></span>
            <div class="ext-copy">
              <h2>{{ item.name }} <small>{{ item.version }}</small></h2>
              <p>{{ item.summary || "没有说明" }}</p>
            </div>
            <footer>
              <span>{{ item.author || "Firefox 附加组件" }} · {{ usersLabel(item.users) }}</span>
              <button class="button secondary" :disabled="busy || added(item)" @click="addStore(item)">
                {{ added(item) ? "已添加" : "添加" }}
              </button>
            </footer>
          </article>
        </div>
        <div v-else-if="!storeBusy" class="ext-empty">
          <Puzzle :size="28" />
          <h3>没有找到附加组件</h3>
          <p>换一个词，或粘贴 addons.mozilla.org 的详情页。</p>
        </div>
        <div v-if="store && store.pageCount > 1" class="ext-pager">
          <button class="button secondary" :disabled="storeBusy || store.page <= 1" @click="loadStore(store.page - 1)">上一页</button>
          <span>{{ store.page }} / {{ store.pageCount }}</span>
          <button class="button secondary" :disabled="storeBusy || store.page >= store.pageCount" @click="loadStore(store.page + 1)">下一页</button>
        </div>
      </div>
      <div v-else-if="rows.length" class="ext-grid">
        <article v-for="item in rows" :key="item.id" class="ext-card">
          <img v-if="item.icon" :src="item.icon" alt="" />
          <span v-else class="ext-fallback"><Puzzle :size="22" /></span>
          <div class="ext-copy">
            <h2>{{ item.name }} <small>{{ item.version }}</small></h2>
            <p>{{ item.description || "没有扩展说明" }}</p>
          </div>
          <footer>
            <span>{{ extensionBroken(item) ? item.reason : sourceLabel(item.source) }}</span>
            <div class="ext-actions">
              <template v-if="!extensionBroken(item)">
                <button class="ext-switch" role="switch" :aria-checked="item.scope === 'all'" :aria-label="`${item.name} 加载到所有窗口`" :disabled="busy" @click="toggle(item)">
                  <i></i>
                </button>
                <button class="text-button" @click="openAssign(item.id)">指定窗口</button>
              </template>
              <button class="icon-button" :aria-label="`${item.name} 更多操作`" @click="menu = menu === item.id ? '' : item.id">
                <MoreHorizontal :size="16" />
              </button>
              <div v-if="menu === item.id" class="ext-menu">
                <button :disabled="busy" @click="run('extension_update', { id: item.id }, '已重新读取扩展')"><RefreshCw :size="15" />更新</button>
                <button @click="removeId = item.id; menu = ''"><Trash2 :size="15" />删除</button>
              </div>
            </div>
          </footer>
          <small v-if="item.scope === 'profiles'" class="ext-badge">指定 {{ item.profileIds.length }} 个窗口</small>
          <small v-else-if="item.scope === 'all'" class="ext-badge on">所有窗口</small>
        </article>
      </div>
      <div v-else class="ext-empty">
        <Puzzle :size="28" />
        <h3>{{ query || status ? "没有符合条件的扩展" : view === "broken" ? "没有不可用扩展" : "还没有扩展" }}</h3>
        <p v-if="view === 'mine' && !query && !status">从 Firefox 商店添加，或选择本地目录和安装包。Chrome 商店扩展不能在当前 Camoufox 内核运行。</p>
      </div>
    </template>

    <Teleport to="body">
      <div v-if="urlOpen" class="modal-overlay" @click.self="urlOpen = false">
        <form class="dialog" @submit.prevent="importUrl">
          <h2>添加 Firefox 附加组件</h2>
          <p class="dialog-description">打开 addons.mozilla.org 的扩展详情页，粘贴地址栏里的链接。星相会下载一份安装包，供所有窗口共用。</p>
          <label class="form-field">详情页地址
            <input v-model="url" placeholder="https://addons.mozilla.org/firefox/addon/..." required />
          </label>
          <p v-if="error" class="form-error" role="alert">{{ error }}</p>
          <footer class="dialog-footer">
            <button type="button" class="button secondary" @click="urlOpen = false">取消</button>
            <button class="button primary" :disabled="busy">识别并添加</button>
          </footer>
        </form>
      </div>
      <div v-if="assigning" class="modal-overlay" @click.self="assignId = ''">
        <form class="dialog ext-assign" @submit.prevent="saveAssign('profiles')">
          <h2>指定窗口 · {{ assigning.name }}</h2>
          <p class="dialog-description">未勾选的窗口不会加载它。选择「所有窗口」则忽略勾选。</p>
          <div v-if="profiles.length" class="ext-profiles">
            <label v-for="profile in profiles" :key="profile.id">
              <input v-model="selected" type="checkbox" :value="profile.id" />
              <span>{{ profile.serial }}</span>{{ profile.name }}
            </label>
          </div>
          <p v-else class="ext-note">还没有浏览器窗口。可以先设为所有窗口，之后新建的窗口也会加载。</p>
          <footer class="dialog-footer">
            <button type="button" class="button secondary" @click="assignId = ''">取消</button>
            <button type="button" class="button secondary" :disabled="busy" @click="saveAssign('off')">不加载</button>
            <button type="button" class="button secondary" :disabled="busy" @click="saveAssign('all')">所有窗口</button>
            <button class="button primary" :disabled="busy || !selected.length">保存指定</button>
          </footer>
        </form>
      </div>
      <div v-if="removing" class="modal-overlay" @click.self="removeId = ''">
        <div class="dialog">
          <h2>删除 {{ removing.name }}？</h2>
          <p class="dialog-description">之后打开的窗口不再加载它。星相自己复制的那一份代码会删除；你电脑上原来的目录不会动。各窗口里已经保存的插件数据仍留在对应环境中。</p>
          <footer class="dialog-footer">
            <button class="button secondary" @click="removeId = ''"><X :size="14" />取消</button>
            <button class="button danger" :disabled="busy" @click="confirmRemove">删除</button>
          </footer>
        </div>
      </div>
    </Teleport>
  </section>
</template>
