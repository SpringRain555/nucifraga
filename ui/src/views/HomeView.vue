<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Home, type Issue } from "../api";
import { t } from "../i18n/zh-TW";
import { formatDate } from "../format";
import OutcomeBadge from "../components/OutcomeBadge.vue";
import IssueList from "../components/IssueList.vue";

const emit = defineEmits<{
  start: [path: string, title: string];
  edit: [path: string];
  create: [folder: string];
  history: [path: string, title: string];
  versions: [path: string, title: string];
  settings: [];
  folder: [current: string | null, suggested: string, inFolder: number];
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
  const h = home.value;
  if (!h) return;
  if (h.profilesFolder) emit("create", h.profilesFolder);
  // 設定檔損毀重建之後會走到這裡：先選資料夾
  else emit("folder", null, h.suggestedFolder, 0);
}

async function openFolder() {
  try {
    await api.openProfilesFolder();
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
        <header class="card__head">
          <h2>
            {{ c.title }}
            <span v-if="c.external" class="tag">{{ t("home.card.external") }}</span>
          </h2>
          <template v-if="!c.error">
            <span :class="c.destinationConnected ? 'ok' : 'warn'" class="status">
              {{ c.destinationConnected ? "●" : "○" }}
              {{ c.destinationConnected ? t("home.card.connected") : t("home.card.notConnected") }}
            </span>
            <button type="button" class="primary" :disabled="home.running" @click="emit('start', c.path, c.title)">
              {{ t("home.card.start") }}
            </button>
          </template>
        </header>

        <template v-if="c.error">
          <p class="bad">{{ t("home.card.broken") }}</p>
          <IssueList :issues="[c.error]" />
          <p class="muted path">{{ c.path }}</p>
          <div class="card__foot">
            <!-- 外部模板到設定頁「重新指定」或移除；資料夾裡的就是那個檔案，打開資料夾自己修 -->
            <button v-if="c.external" type="button" class="link" @click="emit('settings')">
              {{ t("home.card.fixInSettings") }}
            </button>
            <button v-else type="button" class="link" @click="openFolder">{{ t("home.openFolder") }}</button>
          </div>
        </template>

        <template v-else>
          <dl class="facts">
            <dt>{{ t("home.card.sources") }}</dt>
            <dd>
              <ul class="paths">
                <li v-for="r in c.roots" :key="r" class="path">{{ r }}</li>
              </ul>
            </dd>
            <dt>{{ t("home.card.destination") }}</dt>
            <dd class="path">{{ c.destination }}</dd>
            <dt>{{ t("home.card.lastLabel") }}</dt>
            <dd>
              <template v-if="c.last">
                {{ formatDate(c.last.finishedAt) }}
                <OutcomeBadge :outcome="c.last.outcome" />
              </template>
              <span v-else-if="!c.destinationConnected" class="muted">{{ t("home.card.unknownWhileDisconnected") }}</span>
              <span v-else class="muted">{{ t("home.card.never") }}</span>
            </dd>
            <dt>{{ t("home.card.file") }}</dt>
            <dd class="path muted">{{ c.path }}</dd>
          </dl>
          <div class="card__foot">
            <button type="button" class="link" @click="emit('edit', c.path)">{{ t("home.card.edit") }}</button>
            <button type="button" class="link" :disabled="!c.destinationConnected" @click="emit('history', c.path, c.title)">
              {{ t("home.card.history") }}
            </button>
            <button
              type="button"
              class="link"
              :disabled="!c.destinationConnected"
              @click="emit('versions', c.path, c.title)"
            >
              {{ t("home.card.versions") }}
            </button>
          </div>
        </template>
      </li>
    </ul>

    <div class="actions">
      <button type="button" class="primary" @click="createNew">{{ t("home.createNew") }}</button>
      <button type="button" @click="addExisting">{{ t("home.addExisting") }}</button>
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
  display: grid;
  gap: 12px;
  padding: 0;
  overflow: hidden;
}
.card__head {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 16px;
  padding: 16px 20px 0;
}
.card__head h2 {
  margin: 0;
  font-size: 1.2rem;
  margin-right: auto;
}
.card > p,
.card > :deep(.issues) {
  margin: 0 20px;
}
.status {
  font-size: 0.9rem;
  white-space: nowrap;
}
.facts {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 6px 20px;
  margin: 0;
  padding: 0 20px;
}
.facts dt {
  color: var(--muted);
}
.facts dd {
  margin: 0;
  min-width: 0;
}
.paths {
  list-style: none;
  margin: 0;
  padding: 0;
}
.path {
  word-break: break-all;
  font-family: "Cascadia Mono", Consolas, "Microsoft JhengHei UI", monospace;
  font-size: 0.92rem;
}
.card__foot {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  padding: 10px 20px;
  border-top: 1px solid var(--border);
  background: var(--bg);
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
.empty {
  margin-bottom: 20px;
}
</style>
