import { describe, expect, it } from "vitest";
import {
  createProfile,
  initialWorkspace,
  parseWorkspace,
  removeGroup,
  removeProxy,
  visibleProfiles,
} from "./model";

describe("workspace operations", () => {
  it("round-trips backups and rejects invalid references and duplicate IDs", () => {
    const w = initialWorkspace();
    expect(parseWorkspace(JSON.stringify(w))).toEqual(w);
    w.profiles[0]!.groupId = "missing";
    expect(() => parseWorkspace(JSON.stringify(w))).toThrow("失效引用");
    const duplicate = initialWorkspace();
    duplicate.profiles.push(duplicate.profiles[0]!);
    expect(() => parseWorkspace(JSON.stringify(duplicate))).toThrow("ID");
  });
  it("combines search, group, favorites and archive filters", () => {
    const w = initialWorkspace();
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
    const w = initialWorkspace();
    removeGroup(w, "web3");
    removeProxy(w, "us");
    expect(
      w.profiles.some((p) => p.groupId === "web3" || p.proxyId === "us"),
    ).toBe(false);
    expect(() => parseWorkspace(JSON.stringify(w))).not.toThrow();
  });
  it("never reuses a serial number after archiving", () => {
    const w = initialWorkspace();
    w.profiles[7]!.archived = true;
    expect(createProfile(w).serial).toBe(9);
  });
});
