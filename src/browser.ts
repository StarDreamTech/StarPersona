import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import { desktop } from "./workspace";

export interface BrowserSettings {
  executable: string;
  addons: string[];
}
interface BrowserStatus {
  settings: BrowserSettings;
  path: string | null;
  version: string | null;
  instances: { profileId: string; pid: number; stopping: boolean }[];
  errors: Record<string, string>;
}
export const browser = ref<BrowserStatus>({
  settings: { executable: "", addons: [] },
  path: null,
  version: null,
  instances: [],
  errors: {},
});
export async function refreshBrowser() {
  if (desktop) browser.value = await invoke<BrowserStatus>("browser_status");
}
export async function toggleBrowserProcess(profileId: string) {
  if (!desktop) throw new Error("请在桌面应用中启动浏览器");
  const running = browser.value.instances.some((p) => p.profileId === profileId);
  browser.value = await invoke<BrowserStatus>(running ? "stop_browser" : "launch_browser", { profileId });
}
export async function saveBrowserSettings(settings: BrowserSettings) {
  if (!desktop) throw new Error("请在桌面应用中设置本地内核");
  browser.value = await invoke<BrowserStatus>("save_browser_settings", { settings });
}
