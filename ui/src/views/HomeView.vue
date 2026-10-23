<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Home, type Issue } from "../api";
import { t } from "../i18n/zh-TW";
import { formatDate } from "../format";
import OutcomeBadge from "../components/OutcomeBadge.vue";
import IssueList from "../components/IssueList.vue";

const emit = defineEmits<{
  start: [path: string, title: string];
  edit: [path: string];
  create: [folder: string];
  folder: [current: string | null, suggested: string, inFolder: number];
  history: [path: string, title: string];
  versions: [path: string, title: string];
}>();

const home = ref<Home | null>(null);
const error = ref<Issue | null>(null);

async function refresh() {
  try {
    home.value = await api.home();
  } catch (e) {
    error.value = asIssue(e);
  }
}

const jsonFilter = [{ name: t("home.jsonFilter"), extensions: ["json"] }];

async function addExisting() {
  const path = await open({ title: t("home.pickProfile"), filters: jsonFilter, multiple: false, directory: false });
  if (typeof path !== "string") return;
  try {
    await api.addProfile(path);
    error.value = null;
    await refresh();
  } catch (e) {
    error.value = asIssue(e);
  }
}

function createNew() {
  if (home.value?.profilesFolder) emit("create", home.value.profilesFolder);
  else changeFolder(); // 設定檔損毀重建之後會走到這裡：先選資料夾
}

function changeFolder() {
  const h = home.value;
  if (!h) return;
  emit("folder", h.profilesFolder ?? null, h.suggestedFolder, h.profiles.filter((p) => !p.external).length);
}

async function openFolder() {
  try {
    await api.openProfilesFolder();
  } catch (e) {
    error.value = asIssue(e);
  }
}

async function forget(path: string, title: string) {
  const yes = await ask(t("home.card.forgetConfirm", { title }), {
    title: t("app.title"),
    kind: "warning",
    okLabel: t("home.card.forgetButton"),
    cancelLabel: t("home.card.keep"),
  });
  if (!yes) return;
  await api.forgetProfile(path);
  await refresh();
}

async function relocate(oldPath: string) {
  const path = await open({ title: t("home.pickProfile"), filters: jsonFilter, multiple: false, directory: false });
  if (typeof path !== "string") return;
  try {
    await api.addProfile(path);
    await api.forgetProfile(oldPath);
    error.value = null;
    await refresh();
  } catch (e) {
    error.value = asIssue(e);
  }
}

onMounted(refresh);
defineExpose({ refresh });
</script>

<template>
  <section>
    <p v-if="home?.settingsWasBroken" class="notice">{{ t("app.settingsBroken") }}</p>
    <IssueList v-if="error" :issues="[error]" />
    <IssueList v-if="home?.folderIssue" :issues="[home.folderIssue]" />

    <div v-if="home && home.profiles.length === 0" class="panel empty">
      <h2>{{ t("home.empty.title") }}</h2>
      <p>{{ t("home.empty.body") }}</p>
    </div>

    <ul v-if="home" class="cards">
      <li v-for="c in home.profiles" :key="c.path" class="panel card">
        <div class="card__main">
          <h2>
            {{ c.title }}
            <span v-if="c.external" class="tag">{{ t("home.card.external") }}</span>
          </h2>
          <template v-if="c.error">
            <p class="bad">{{ t("home.card.broken") }}</p>
            <IssueList :issues="[c.error]" />
          </template>
          <template v-else>
            <p class="muted">{{ t("home.card.destination", { path: c.destination ?? "" }) }}</p>
            <p class="muted">{{ t("home.card.sources", { count: c.roots.length }) }}</p>
            <p v-if="!c.destinationConnected" class="warn">⚠ {{ t("home.card.notConnected") }}</p>
            <p v-else-if="c.last">
              {{ t("home.card.last", { when: formatDate(c.last.finishedAt) }) }}
              <OutcomeBadge :outcome="c.last.outcome" />
            </p>
            <p v-else class="muted">{{ t("home.card.never") }}</p>
          </template>
        </div>
        <div class="card__actions">
          <template v-if="!c.error">
            <button type="button" class="primary" :disabled="home.running" @click="emit('start', c.path, c.title)">
              {{ t("home.card.start") }}
            </button>
            <button type="button" @click="emit('edit', c.path)">{{ t("home.card.edit") }}</button>
            <button type="button" :disabled="!c.destinationConnected" @click="emit('history', c.path, c.title)">
              {{ t("home.card.history") }}
            </button>
            <button type="button" :disabled="!c.destinationConnected" @click="emit('versions', c.path, c.title)">
              {{ t("home.card.versions") }}
            </button>
          </template>
          <!-- 資料夾裡的模板沒有「重新指定」與「從清單移除」：首頁列的就是資料夾內容，要移除就把檔案搬走 -->
          <button v-else-if="c.external" type="button" class="primary" @click="relocate(c.path)">
            {{ t("home.card.relocate") }}
          </button>
          <button v-if="c.external" type="button" class="link" @click="forget(c.path, c.title)">
            {{ t("home.card.forget") }}
          </button>
        </div>
      </li>
    </ul>

    <div class="actions">
      <button type="button" class="primary" @click="addExisting">{{ t("home.addExisting") }}</button>
      <button type="button" @click="createNew">{{ t("home.createNew") }}</button>
    </div>

    <div v-if="home?.profilesFolder" class="folder">
      <p class="muted">{{ t("home.folder", { path: home.profilesFolder }) }}</p>
      <div class="actions">
        <button type="button" class="link" @click="openFolder">{{ t("home.openFolder") }}</button>
        <button type="button" class="link" :disabled="home.running" @click="changeFolder">
          {{ t("home.changeFolder") }}
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.cards {
  list-style: none;
  padding: 0;
  margin: 0 0 20px;
  display: grid;
  gap: 14px;
}
.card {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}
.card h2 {
  margin: 0 0 4px;
  font-size: 1.2rem;
}
.card p {
  margin: 2px 0;
}
.card__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: flex-start;
}
.empty {
  margin-bottom: 20px;
}
.tag {
  font-size: 0.8rem;
  font-weight: normal;
  color: var(--muted);
  border: 1px solid var(--muted);
  border-radius: 4px;
  padding: 0 6px;
  margin-left: 8px;
  vertical-align: middle;
}
.folder {
  margin-top: 24px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  align-items: center;
}
.folder p {
  margin: 0;
  word-break: break-all;
}
</style>
