<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { t } from "./i18n/zh-TW";

interface AppInfo {
  version: string;
  identifier: string;
}

const info = ref<AppInfo | null>(null);

onMounted(async () => {
  info.value = await invoke<AppInfo>("app_info");
  // 等瀏覽器真的畫出第一格再寫就緒標記：啟動器看到標記就會關掉主控台（app-lifecycle.md 第 3 題）。
  requestAnimationFrame(() => {
    void invoke("app_ready");
  });
});
</script>

<template>
  <main class="home">
    <header class="home__header">
      <h1>{{ t("app.title") }}</h1>
      <span v-if="info" class="home__version">{{ t("app.version", { version: info.version }) }}</span>
    </header>

    <section class="empty">
      <h2>{{ t("home.empty.title") }}</h2>
      <p>{{ t("home.empty.body") }}</p>
      <div class="empty__actions">
        <button type="button" disabled>{{ t("home.empty.addExisting") }}</button>
        <button type="button" disabled>{{ t("home.empty.createNew") }}</button>
      </div>
      <p class="empty__note">{{ t("home.notReady") }}</p>
    </section>
  </main>
</template>
