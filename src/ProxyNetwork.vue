<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { Download } from "lucide-vue-next";
import { commit, desktop, ready, storageError } from "./workspace";
import {
  endpoint,
  mergeNodes,
  snapshotKey,
  type Listing,
  type Measure,
  type Tested,
} from "./starport";
import { browser } from "./browser";
import type { Proxy } from "./model";
const busy = ref(false),
  message = ref("");
const rowBusy = ref("");
let disposed = false;
onBeforeUnmount(() => {
  disposed = true;
});
const list = () => invoke<Listing>("starport_list");
const probe = (p: Pick<Proxy, "host" | "protocol" | "port">, speed = false) =>
  invoke<Measure>("proxy_measure", {
    host: p.host,
    port: p.port,
    protocol: p.protocol,
    speed,
  });
async function sync() {
  if (busy.value || !desktop || !ready.value) return;
  busy.value = true;
  message.value = "正在读取本机 StarPort…";
  try {
    const first = await list();
    if (!first.nodes.length) {
      message.value = `没有可导入的监听端口，跳过 ${first.skipped} 项。已有代理保持不变。`;
      return;
    }
    const rows: Tested[] = [];
    let next = 0;
    async function worker() {
      while (!disposed && next < first.nodes.length) {
        const n = first.nodes[next++]!;
        let ms = null;
        try {
          ms = (
            await probe({ host: "127.0.0.1", port: n.port, protocol: "SOCKS5" })
          ).ms;
        } catch {
          /* skip unhealthy entries */
        }
        rows.push({ ...n, ms });
        message.value = `正在检测 ${rows.length}/${first.nodes.length}…`;
      }
    }
    await Promise.all(
      Array.from({ length: Math.min(4, first.nodes.length) }, worker),
    );
    if (disposed) return;
    const second = await list();
    if (disposed) return;
    const stable = new Set(second.nodes.map(snapshotKey));
    const valid = rows.filter((n) => stable.has(snapshotKey(n)));
    const skipped = first.skipped + rows.length - valid.length;
    let result = "";
    if (!valid.some((n) => n.ms !== null)) {
      message.value = `没有通过检测的稳定端口，已有代理保持不变。失败 ${valid.length}，跳过 ${skipped}。`;
      return;
    }
    const saved = await commit((w) => {
      const live = new Set(browser.value.instances.map((i) => i.profileId));
      const inUse = new Set(
        w.profiles.filter((p) => live.has(p.id)).map((p) => p.proxyId),
      );
      const c = mergeNodes(w, valid, new Date().toISOString(), inUse);
      result = `新增 ${c.added}，更新 ${c.updated}，已存在 ${c.unchanged}，检测失败 ${c.failed}，其他跳过 ${skipped}，端口冲突 ${c.conflict}，使用中暂缓换端口 ${c.inUse}。${valid.some((n) => !n.nodeId) ? "当前星口未提供节点 UUID，按本地地址去重。" : ""}`;
    });
    message.value = saved ? result : storageError.value;
  } catch (e) {
    message.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function test(p: Proxy, speed: boolean) {
  if (busy.value || !desktop || !ready.value) return;
  const key = endpoint(p);
  busy.value = true;
  rowBusy.value = p.id;
  message.value = speed
    ? "正在通过此代理下载 1 MiB 测试数据…"
    : "正在测试 HTTPS 连通性…";
  try {
    let m: Measure | null = null,
      error = "";
    try {
      m = await probe(p, speed);
    } catch (e) {
      error = String(e);
    }
    if (disposed) return;
    if (!m && speed) {
      message.value = `测速失败：${error}；保留上次成功结果。`;
      return;
    }
    const at = new Date().toISOString();
    let stale = false;
    const saved = await commit((w) => {
      const x = w.proxies.find((i) => i.id === p.id);
      if (!x || endpoint(x) !== key) {
        stale = true;
        return;
      }
      x.check = {
        ...x.check,
        at,
        ok: !!m,
        message: error,
        ...(m && !speed ? { lastGoodMs: m.ms } : {}),
        ...(m && speed && m.bytesPerSecond !== null
          ? { bytesPerSecond: m.bytesPerSecond, speedAt: at }
          : {}),
      };
    });
    message.value = !saved
      ? storageError.value
      : stale
        ? "代理已改变，丢弃旧检测结果。"
        : m
          ? speed
            ? `下载速度 ${(((m.bytesPerSecond ?? 0) * 8) / 1e6).toFixed(2)} Mbps（含建连）`
            : `HTTPS 连通，耗时 ${m.ms} ms`
          : `连通测试失败：${error}；保留代理配置。`;
  } catch (e) {
    message.value = String(e);
  } finally {
    rowBusy.value = "";
    busy.value = false;
  }
}
defineExpose({ test, busy, rowBusy });
</script>
<template>
  <div class="starport-tools">
    <button
      class="button secondary"
      :disabled="busy || !desktop || !ready"
      @click="sync"
    >
      <Download :size="16" />{{
        busy && !rowBusy ? "正在导入…" : "从 StarPort 导入代理列表"
      }}</button
    ><small
      >只导入通过 HTTPS 检测的端口，不删除已有代理。测速每次下载 1 MiB。</small
    >
    <p role="status" aria-live="polite" v-if="message || !desktop">
      {{ message || "请在桌面版使用导入和测速。" }}
    </p>
  </div>
</template>
<style scoped>
.starport-tools {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin: 0 0 12px;
}
.starport-tools small {
  color: #54637a;
}
.starport-tools p {
  width: 100%;
  margin: 0;
  color: #354864;
}
</style>
