import { invoke, isTauri } from "@tauri-apps/api/core";
import { ref } from "vue";
import {
  initialWorkspace,
  parseWorkspace,
  stripDemoData,
  type Workspace,
} from "./model";

const key = "starpersona.workspace.v1";
export const workspace = ref<Workspace>(initialWorkspace());
export const ready = ref(false);
export const saving = ref(false);
export const storageError = ref("");
export const desktop = isTauri();
let queue = Promise.resolve(true);

async function persist(value: Workspace, expected: Workspace | null = null) {
  parseWorkspace(JSON.stringify(value));
  if (desktop) await invoke("save_workspace", { workspace: value, expected });
  else localStorage.setItem(key, JSON.stringify(value));
}
export async function loadWorkspace() {
  try {
    const saved = desktop
      ? await invoke<Workspace | null>("read_workspace")
      : localStorage.getItem(key);
    storageError.value = "";
    if (saved) {
      workspace.value = parseWorkspace(
        typeof saved === "string" ? saved : JSON.stringify(saved),
      );
      const previous = JSON.parse(
        JSON.stringify(workspace.value),
      ) as Workspace;
      if (stripDemoData(workspace.value)) {
        try {
          await persist(workspace.value, previous);
        } catch (error) {
          storageError.value = `示例数据清理未保存：${String(error)}`;
        }
      }
    } else await persist(workspace.value);
    ready.value = true;
  } catch (error) {
    storageError.value = `工作空间读取失败：${String(error)}。请检查存储后重试，原数据不会被覆盖。`;
  }
}
export function commit(update: (draft: Workspace) => void): Promise<boolean> {
  queue = queue.then(async () => {
    if (!ready.value) return false;
    saving.value = true;
    try {
      const next = JSON.parse(JSON.stringify(workspace.value)) as Workspace;
      update(next);
      await persist(next, workspace.value);
      workspace.value = next;
      storageError.value = "";
      return true;
    } catch (error) {
      storageError.value = `保存失败：${String(error)}`;
      return false;
    } finally {
      saving.value = false;
    }
  });
  return queue;
}

export async function reloadWorkspace() {
  await queue;
  await loadWorkspace();
}
