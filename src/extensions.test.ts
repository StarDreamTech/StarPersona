import { describe, expect, it } from "vitest";
import {
  classifyExtensionUrl,
  filterExtensions,
  type ManagedExtension,
} from "./extensions";

function item(patch: Partial<ManagedExtension>): ManagedExtension {
  return {
    id: "probe@starpersona.local",
    name: "隔离探针",
    description: "测试扩展",
    version: "1.0.0",
    icon: "",
    path: "/extensions/probe",
    source: "local",
    sourceUrl: "",
    origin: "/tmp/probe",
    scope: "off",
    profileIds: [],
    engine: "firefox",
    available: true,
    reason: "",
    ...patch,
  };
}

describe("extension center filters", () => {
  const items = [
    item({}),
    item({ id: "on@local", name: "全开", scope: "all" }),
    item({ id: "some@local", name: "指定", scope: "profiles", profileIds: ["a"] }),
    item({
      id: "chromium-1",
      name: "Chrome Only",
      engine: "chromium",
      available: false,
      reason: "没有 Firefox 扩展 ID",
    }),
  ];

  it("separates usable extensions from ones the current kernel cannot load", () => {
    expect(filterExtensions(items, "mine", "", "").map((entry) => entry.id)).toEqual([
      "probe@starpersona.local",
      "on@local",
      "some@local",
    ]);
    expect(filterExtensions(items, "broken", "chrome", "").map((entry) => entry.name)).toEqual([
      "Chrome Only",
    ]);
  });

  it("filters by load scope and name", () => {
    expect(filterExtensions(items, "mine", "指定", "profiles")).toHaveLength(1);
    expect(filterExtensions(items, "mine", "", "all").map((entry) => entry.name)).toEqual(["全开"]);
  });

  it("recognizes Firefox add-on pages and rejects the Chrome Web Store", () => {
    expect(
      classifyExtensionUrl("https://addons.mozilla.org/zh-CN/firefox/addon/ublock-origin/"),
    ).toBe("amo");
    expect(
      classifyExtensionUrl(
        "https://chromewebstore.google.com/detail/metamask/nkbihfbeogaeaoehlefnkodbefgpgknn",
      ),
    ).toBe("chrome");
    expect(classifyExtensionUrl("http://addons.mozilla.org/firefox/addon/ublock-origin/")).toBe(
      "invalid",
    );
  });
});
