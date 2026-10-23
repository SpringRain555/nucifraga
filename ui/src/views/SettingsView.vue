<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Diagnostics, type Home, type Issue } from "../api";
import { t } from "../i18n/zh-TW";
import IssueList from "../components/IssueList.vue";

// 設定：模板資料夾、外部模板、關於與診斷（ui-workflows.md「設定」）。
const emit = defineEmits<{
  folder: [current: string | null, suggested: string, inFolder: number];
}>();

const home = ref<Home | null>(null);
const diag = ref<Diagnostics | null>(null);
const error = ref<Issue | null>(null);

const externals = computed(() => home.value?.profiles.filter((p) => p.external) ?? []);

async function refresh() {
  try {
    [home.value, diag.value] = await Promise.all([api.home(), api.diagnostics()]);
  } catch (e) {
    error.value = asIssue(e);
  }
}
onMounted(refresh);

async function run(f: () => Promise<void>) {
  try {
    error.value = null;
    await f();
  } catch (e) {
    error.value = asIssue(e);
  }
}

function changeFolder() {
  const h = home.value;
  if (!h) return;
  emit("folder", h.profilesFolder ?? null, h.suggestedFolder, h.profiles.filter((p) => !p.external).length);
}

const openProfilesFolder = () => run(api.openProfilesFolder);
const openAppFolder = (which: "settings" | "reports") => run(() => api.openAppFolder(which));

const jsonFilter = [{ name: t("home.jsonFilter"), extensions: ["json"] }];

async function pickJson(): Promise<string | null> {
  const p = await open({ title: t("home.pickProfile"), filters: jsonFilter, multiple: false, directory: false });
  return typeof p === "string" ? p : null;
}

async function toggleBackground(e: Event) {
  const on = (e.target as HTMLInputElement).checked;
  await run(async () => {
    await api.setBackgroundMode(on);
    if (diag.value) diag.value.backgroundMode = on;
  });
}

async function addExisting() {
  const p = await pickJson();
  if (p) await run(async () => (await api.addProfile(p), await refresh()));
}

async function relocate(oldPath: string) {
  const p = await pickJson();
  if (p) await run(async () => (await api.addProfile(p), await api.forgetProfile(oldPath), await refresh()));
}

async function forget(path: string, title: string) {
  const yes = await ask(t("home.card.forgetConfirm", { title }), {
    title: t("app.title"),
    kind: "warning",
    okLabel: t("home.card.forgetButton"),
    cancelLabel: t("home.card.keep"),
  });
  if (yes) await run(async () => (await api.forgetProfile(path), await refresh()));
}
</script>

<template>
  <section class="settings">
    <h2>{{ t("settings.title") }}</h2>
    <IssueList v-if="error" :issues="[error]" />

    <section class="panel block">
      <h3>{{ t("settings.folder.title") }}</h3>
      <p class="muted">{{ t("settings.folder.help") }}</p>
      <p class="path">{{ home?.profilesFolder }}</p>
      <IssueList v-if="home?.folderIssue" :issues="[home.folderIssue]" />
      <div class="actions">
        <button type="button" @click="openProfilesFolder">{{ t("home.openFolder") }}</button>
        <button type="button" :disabled="home?.running" @click="changeFolder">{{ t("home.changeFolder") }}</button>
      </div>
    </section>

    <section class="panel block">
      <h3>{{ t("settings.external.title") }}</h3>
      <p class="muted">{{ t("settings.external.help") }}</p>
      <p v-if="externals.length === 0" class="muted">{{ t("settings.external.empty") }}</p>
      <ul v-else class="externals">
        <li v-for="p in externals" :key="p.path">
          <div class="externals__main">
            <strong>{{ p.title }}</strong>
            <span v-if="p.error" class="bad">{{ t("home.card.broken") }}</span>
            <span class="path muted">{{ p.path }}</span>
          </div>
          <div class="externals__actions">
            <button type="button" class="link" @click="relocate(p.path)">{{ t("home.card.relocate") }}</button>
            <button type="button" class="link" @click="forget(p.path, p.title)">{{ t("home.card.forget") }}</button>
          </div>
        </li>
      </ul>
      <div class="actions">
        <button type="button" @click="addExisting">{{ t("home.addExisting") }}</button>
      </div>
    </section>

    <section class="panel block">
      <h3>{{ t("settings.speed.title") }}</h3>
      <label class="check">
        <input type="checkbox" :checked="diag?.backgroundMode ?? true" @change="toggleBackground" />
        {{ t("settings.speed.background") }}
      </label>
      <p class="muted">{{ t("settings.speed.help") }}</p>
    </section>

    <section class="panel block">
      <h3>{{ t("settings.about.title") }}</h3>
      <dl v-if="diag" class="facts">
        <dt>{{ t("settings.about.version") }}</dt>
        <dd>{{ diag.version }}</dd>
        <dt>{{ t("settings.about.identifier") }}</dt>
        <dd class="path">{{ diag.identifier }}</dd>
        <dt>{{ t("settings.about.settingsFile") }}</dt>
        <dd>
          <span class="path">{{ diag.settingsFile }}</span>
          <button type="button" class="link" @click="openAppFolder('settings')">
            {{ t("settings.about.open") }}
          </button>
        </dd>
        <dt>{{ t("settings.about.fallbackReports") }}</dt>
        <dd>
          <span class="path">{{ diag.fallbackReports }}</span>
          <button type="button" class="link" @click="openAppFolder('reports')">
            {{ t("settings.about.open") }}
          </button>
          <p class="muted hint">{{ t("settings.about.fallbackHelp") }}</p>
        </dd>
      </dl>
    </section>
  </section>
</template>

<style scoped>
.settings {
  display: grid;
  gap: 16px;
}
.settings > h2 {
  margin: 0;
}
.block {
  display: grid;
  gap: 10px;
}
.block h3,
.block p {
  margin: 0;
}
.path {
  word-break: break-all;
  font-family: "Cascadia Mono", Consolas, "Microsoft JhengHei UI", monospace;
  font-size: 0.92rem;
}
.externals {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 8px;
}
.externals li {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 8px 16px;
  padding: 8px 0;
  border-top: 1px solid var(--border);
}
.externals__main {
  display: grid;
  min-width: 0;
}
.externals__actions {
  display: flex;
  gap: 16px;
  align-items: flex-start;
}
.facts {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 6px 20px;
  margin: 0;
}
.facts dt {
  color: var(--muted);
}
.facts dd {
  margin: 0;
  min-width: 0;
}
.facts dd .link {
  margin-left: 12px;
}
.hint {
  font-size: 0.85rem;
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
}
</style>
