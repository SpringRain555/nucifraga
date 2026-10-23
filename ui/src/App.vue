<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";
import { ask } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { t } from "./i18n/zh-TW";
import HomeView from "./views/HomeView.vue";
import EditView from "./views/EditView.vue";
import RunView from "./views/RunView.vue";
import HistoryView from "./views/HistoryView.vue";
import VersionsView from "./views/VersionsView.vue";
import FolderView from "./views/FolderView.vue";

// 畫面只有六個，用一個狀態切換就夠，不引 vue-router（ui-workflows.md「畫面」）。
type Screen =
  | { name: "loading" }
  | { name: "home" }
  | { name: "folder"; current: string | null; suggested: string; inFolder: number }
  | { name: "edit"; path: string; creating: boolean }
  | { name: "run"; path: string; title: string }
  | { name: "history"; path: string; title: string }
  | { name: "versions"; path: string; title: string; destination: string };

const screen = ref<Screen>({ name: "loading" });
const version = ref("");

function goHome() {
  screen.value = { name: "home" };
}

function titleClicked() {
  // 第一次使用還沒選資料夾：首頁沒有東西可以做，按標題也留在這裡。
  if (screen.value.name === "folder" && screen.value.current === null) return;
  goHome();
}

async function openVersions(path: string, title: string) {
  const home = await api.home();
  const destination = home.profiles.find((p) => p.path === path)?.destination ?? "";
  screen.value = { name: "versions", path, title, destination };
}

onMounted(async () => {
  version.value = (await api.info()).version;
  // 第一次使用（還沒選模板資料夾）先選資料夾，再進首頁（ui-workflows.md「模板資料夾」）。
  const home = await api.home();
  screen.value = home.profilesFolder
    ? { name: "home" }
    : { name: "folder", current: null, suggested: home.suggestedFolder, inFolder: 0 };
  // 備份進行中按關閉：後端先攔下來，這裡問使用者（app-lifecycle.md 第 5 題）。
  await api.onCloseRequested(async () => {
    const yes = await ask(t("run.close.body"), {
      title: t("run.close.title"),
      kind: "warning",
      okLabel: t("run.close.confirm"),
      cancelLabel: t("run.close.keep"),
    });
    if (yes) await api.cancelRun(true);
  });
  // 等瀏覽器真的畫出第一格再寫就緒標記：啟動器看到標記就會關掉主控台（app-lifecycle.md 第 3 題）。
  await nextTick();
  requestAnimationFrame(() => {
    void api.ready();
  });
});
</script>

<template>
  <main class="app">
    <header class="app__header">
      <h1>
        <button type="button" class="link title" @click="titleClicked">{{ t("app.title") }}</button>
      </h1>
      <span class="muted">{{ t("app.version", { version }) }}</span>
    </header>

    <HomeView
      v-if="screen.name === 'home'"
      @start="(path, title) => (screen = { name: 'run', path, title })"
      @edit="(path) => (screen = { name: 'edit', path, creating: false })"
      @create="(folder) => (screen = { name: 'edit', path: folder, creating: true })"
      @folder="(current, suggested, inFolder) => (screen = { name: 'folder', current, suggested, inFolder })"
      @history="(path, title) => (screen = { name: 'history', path, title })"
      @versions="openVersions"
    />
    <EditView v-else-if="screen.name === 'edit'" :path="screen.path" :creating="screen.creating" @done="goHome" />
    <RunView v-else-if="screen.name === 'run'" :path="screen.path" :title="screen.title" @done="goHome" />
    <HistoryView v-else-if="screen.name === 'history'" :path="screen.path" :title="screen.title" @done="goHome" />
    <FolderView
      v-else-if="screen.name === 'folder'"
      :current="screen.current"
      :suggested="screen.suggested"
      :in-folder="screen.inFolder"
      @done="goHome"
    />
    <VersionsView
      v-else-if="screen.name === 'versions'"
      :path="screen.path"
      :title="screen.title"
      :destination="screen.destination"
      @done="goHome"
    />
  </main>
</template>

<style scoped>
.app__header {
  display: flex;
  align-items: baseline;
  gap: 12px;
  margin-bottom: 20px;
}
.app__header h1 {
  margin: 0;
  font-size: 1.6rem;
}
.title {
  font-size: inherit;
  font-weight: inherit;
  color: var(--text);
  padding: 0;
}
</style>
