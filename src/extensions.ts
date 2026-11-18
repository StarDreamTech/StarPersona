import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import { desktop } from "./workspace";

export interface ManagedExtension {
  id: string;
  name: string;
  description: string;
  version: string;
  icon: string;
  path: string;
  source: string;
  sourceUrl: string;
  origin: string;
  scope: string;
  profileIds: string[];
  engine: string;
  available: boolean;
  reason: string;
}

export const extensions = ref<ManagedExtension[]>([]);

export function extensionBroken(item: ManagedExtension) {
  return !item.available || item.engine !== "firefox";
}

export function filterExtensions(
  items: ManagedExtension[],
  view: "mine" | "broken",
  query: string,
  status: string,
) {
  const search = query.trim().toLocaleLowerCase();
  return items.filter((item) => {
    if ((view === "broken") !== extensionBroken(item)) return false;
    if (view === "mine" && status && item.scope !== status) return false;
    const text = [item.name, item.description, item.version, item.id]
      .join(" ")
      .toLocaleLowerCase();
    return !search || text.includes(search);
  });
}

export function classifyExtensionUrl(url: string) {
  const value = url.trim();
  const lower = value.toLowerCase();
  if (!value.startsWith("https://")) return "invalid";
  if (
    lower.includes("chrome.google.com/webstore") ||
    lower.includes("chromewebstore.google.com")
  ) {
    return "chrome";
  }
  if (
    lower.includes("addons.mozilla.org/") &&
    /\/addon\/[a-z0-9_-]{1,80}(?:[/?#]|$)/i.test(value)
  ) {
    return "amo";
  }
  return "invalid";
}

export async function refreshExtensions() {
  if (!desktop) return;
  extensions.value = await invoke<ManagedExtension[]>("extension_list");
}

export async function mutateExtensions(command: string, args: Record<string, unknown>) {
  extensions.value = await invoke<ManagedExtension[]>(command, args);
}

export interface StoreAddon {
  slug: string;
  name: string;
  summary: string;
  version: string;
  icon: string;
  author: string;
  users: number;
  pageUrl: string;
  guid: string;
}

export interface StorePage {
  query: string;
  page: number;
  pageCount: number;
  total: number;
  addons: StoreAddon[];
}

export async function searchStore(query: string, page: number) {
  return invoke<StorePage>("extension_store_search", { query, page });
}
