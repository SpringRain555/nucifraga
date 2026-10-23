<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from "vue";
import { ask } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { t } from "./i18n/zh-TW";
import { unsaved } from "./unsaved";
import { initRunEvents } from "./run";
import HomeView from "./views/HomeView.vue";
import EditView from "./views/EditView.vue";
import RunView from "./views/RunView.vue";
import HistoryView from "./views/HistoryView.vue";
import VersionsView from "./views/VersionsView.vue";
import FolderView from "./views/FolderView.vue";
import SettingsView from "./views/SettingsView.vue";
import LeaveDialog from "./components/LeaveDialog.vue";

// 畫面不多，用一個狀態切換就夠，不引 vue-router（ui-workflows.md「畫面」）。
type Screen =
  | { name: "loading" }
  | { name: "home" }
  | { name: "settings" }
  | { name: "folder"; current: string | null; suggested: string; inFolder: number; back: "home" | "settings" }
  | { name: "edit"; path: string; creating: boolean }
  | { name: "run"; path: string; title: string; attach: boolean }
  | { name: "history"; path: string; title: string }
  | { name: "versions"; path: string; title: string; destination: string };

const screen = ref<Screen>({ name: "loading" });
const version = ref("");
/** 等使用者回答「有未儲存的修改」時，答完要去的地方。 */
const pending = ref<Screen | "quit" | null>(null);
const saving = ref(false);

// 第一次使用還沒選資料夾：首頁與設定都沒有東西可以做，不顯示導覽。
const firstRun = computed(() => screen.value.name === "folder" && screen.value.current === null);

/** 所有換頁都走這裡：編輯畫面有未儲存的修改時先問。 */
function go(next: Screen) {
  if (screen.value.name === "edit" && unsaved.dirty) {
    pending.value = next;
    return;
  }
  screen.value = next;
}

function goHome() {
  go({ name: "home" });
}

async function proceed() {
  const target = pending.value;
  pending.value = null;
  unsaved.dirty = false;
  if (target === "quit") await api.quitApp();
  else if (target) screen.value = target;
}

async function saveAndLeave() {
  if (!unsaved.save) return;
  saving.value = true;
  const ok = await unsaved.save();
  saving.value = false;
  // 存不成功就留在編輯畫面，錯誤訊息已經顯示在那裡。
  if (ok) await proceed();
  else pending.value = null;
}

async function openVersions(path: string, title: string) {
  const home = await api.home();
  const destination = home.profiles.find((p) => p.path === path)?.destination ?? "";
  go({ name: "versions", path, title, destination });
}

onMounted(async () => {
  // 備份的事件從 app 一開就收，不跟著執行中畫面掛載與卸載（ui-workflows「進行中」）。
  await initRunEvents();
  version.value = (await api.info()).version;
  // 第一次使用（還沒選模板資料夾）先選資料夾，再進首頁（ui-workflows.md「模板資料夾」）。
  const home = await api.home();
  screen.value = home.profilesFolder
    ? { name: "home" }
    : { name: "folder", current: null, suggested: home.suggestedFolder, inFolder: 0, back: "home" };
  // 按關閉時後端先攔下來：備份進行中問要不要取消；模板沒存問要不要存（app-lifecycle.md 第 5 題）。
  await api.onCloseRequested(async ({ running }) => {
    if (!running) {
      pending.value = "quit";
      return;
    }
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
        <button type="button" class="link title" :disabled="firstRun" @click="goHome">{{ t("app.title") }}</button>
      </h1>
      <span class="muted">{{ t("app.version", { version }) }}</span>
      <nav v-if="!firstRun && screen.name !== 'loading'" class="app__nav">
        <button type="button" class="link" :aria-current="screen.name === 'home' ? 'page' : undefined" @click="goHome">
          {{ t("nav.home") }}
        </button>
        <button
          type="button"
          class="link"
          :aria-current="screen.name === 'settings' ? 'page' : undefined"
          @click="go({ name: 'settings' })"
        >
          {{ t("nav.settings") }}
        </button>
      </nav>
    </header>

    <HomeView
      v-if="screen.name === 'home'"
      @start="(path, title) => go({ name: 'run', path, title, attach: false })"
      @view="(path, title) => go({ name: 'run', path, title, attach: true })"
      @edit="(path) => go({ name: 'edit', path, creating: false })"
      @create="(folder) => go({ name: 'edit', path: folder, creating: true })"
      @history="(path, title) => go({ name: 'history', path, title })"
      @versions="openVersions"
      @settings="go({ name: 'settings' })"
      @folder="(current, suggested, inFolder) => go({ name: 'folder', current, suggested, inFolder, back: 'home' })"
    />
    <SettingsView
      v-else-if="screen.name === 'settings'"
      @folder="(current, suggested, inFolder) => go({ name: 'folder', current, suggested, inFolder, back: 'settings' })"
    />
    <EditView v-else-if="screen.name === 'edit'" :path="screen.path" :creating="screen.creating" @done="goHome" />
    <RunView
      v-else-if="screen.name === 'run'"
      :path="screen.path"
      :title="screen.title"
      :attach="screen.attach"
      @done="goHome"
    />
    <HistoryView v-else-if="screen.name === 'history'" :path="screen.path" :title="screen.title" @done="goHome" />
    <FolderView
      v-else-if="screen.name === 'folder'"
      :current="screen.current"
      :suggested="screen.suggested"
      :in-folder="screen.inFolder"
      @done="go({ name: screen.name === 'folder' ? screen.back : 'home' })"
    />
    <VersionsView
      v-else-if="screen.name === 'versions'"
      :path="screen.path"
      :title="screen.title"
      :destination="screen.destination"
      @done="goHome"
    />

    <LeaveDialog
      v-if="pending"
      :quitting="pending === 'quit'"
      :saving="saving"
      @save="saveAndLeave"
      @discard="proceed"
      @stay="pending = null"
    />
  </main>
</template>

<style scoped>
.app__header {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
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
.title:disabled {
  text-decoration: none;
  cursor: default;
}
.app__nav {
  margin-left: auto;
  display: flex;
  gap: 16px;
}
.app__nav [aria-current="page"] {
  color: var(--text);
  font-weight: 600;
  text-decoration: none;
}
</style>
