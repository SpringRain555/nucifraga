<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Home, type Issue } from "../api";
import { t } from "../i18n/zh-TW";
import { formatDate } from "../format";
import OutcomeBadge from "../components/OutcomeBadge.vue";
import IssueList from "../components/IssueList.vue";

const emit = defineEmits<{
  start: [path: string, title: string];
  edit: [path: string];
  create: [path: string];
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

async function createNew() {
  const path = await save({ title: t("home.saveProfile"), filters: jsonFilter, defaultPath: "nucifraga-模板.json" });
  if (path) emit("create", path);
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

    <div v-if="home && home.profiles.length === 0" class="panel empty">
      <h2>{{ t("home.empty.title") }}</h2>
      <p>{{ t("home.empty.body") }}</p>
    </div>

    <ul v-if="home" class="cards">
      <li v-for="c in home.profiles" :key="c.path" class="panel card">
        <div class="card__main">
          <h2>{{ c.title }}</h2>
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
          <button v-else type="button" class="primary" @click="relocate(c.path)">{{ t("home.card.relocate") }}</button>
          <button type="button" class="link" @click="forget(c.path, c.title)">{{ t("home.card.forget") }}</button>
        </div>
      </li>
    </ul>

    <div class="actions">
      <button type="button" class="primary" @click="addExisting">{{ t("home.addExisting") }}</button>
      <button type="button" @click="createNew">{{ t("home.createNew") }}</button>
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
</style>
