<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Home, type Issue } from "../api";
import { t } from "../i18n/zh-TW";
import { formatDate, formatNumber, formatRemaining, formatSchedule } from "../format";
import { run, isActive, isWaiting, stepOf } from "../run";
import OutcomeBadge from "../components/OutcomeBadge.vue";
import IssueList from "../components/IssueList.vue";

const emit = defineEmits<{
  start: [path: string, title: string];
  edit: [path: string];
  create: [folder: string];
  history: [path: string, title: string];
  versions: [path: string, title: string];
  settings: [];
  view: [path: string, title: string];
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
// 在首頁等的時候備份結束了：卡片上的「上次備份」要跟著更新。
watch(
  () => run.finished,
  () => void refresh(),
);
defineExpose({ refresh });
</script>

<template>
  <section>
    <p v-if="home?.settingsWasBroken" class="notice">{{ t("app.settingsBroken") }}</p>
    <IssueList v-if="error" :issues="[error]" />
    <IssueList v-if="home?.folderIssue" :issues="[home.folderIssue]" />
    <IssueList v-if="home?.scheduleIssues.length" :issues="home.scheduleIssues" />

    <!-- 進行中：離開執行中畫面之後，備份在背景繼續跑；從這裡回去（ui-workflows「進行中」） -->
    <section v-if="isActive()" class="panel active" :class="{ 'active--waiting': isWaiting() }">
      <h2>{{ t("home.active.title") }}</h2>
      <div class="active__body">
        <div>
          <p v-if="run.missingDestination" class="warn">⚠ {{ t("home.active.waitingDest", { title: run.title }) }}</p>
          <p v-else-if="isWaiting()" class="warn">⚠ {{ t("home.active.waiting", { title: run.title }) }}</p>
          <p v-else>
            {{ t("home.active.running", { title: run.title, step: stepOf(run.state) ? t(stepOf(run.state)!) : "" }) }}
          </p>
          <p v-if="run.progress && run.state === 'transferring'" class="muted">
            {{
              t("run.progress.files", {
                done: formatNumber(run.progress.filesDone),
                total: formatNumber(run.progress.filesTotal),
              })
            }}
            · {{ formatRemaining(run.eta) }}
          </p>
          <p v-else-if="run.progress && run.state === 'checkingLocks'" class="muted">
            {{
              t("run.progress.checked", {
                done: formatNumber(run.progress.filesDone),
                total: formatNumber(run.progress.filesTotal),
              })
            }}
          </p>
          <progress
            v-if="run.progress && (run.state === 'transferring' || run.state === 'checkingLocks')"
            :max="Math.max(run.state === 'transferring' ? run.progress.bytesTotal : run.progress.filesTotal, 1)"
            :value="run.state === 'transferring' ? run.progress.bytesDone : run.progress.filesDone"
          ></progress>
        </div>
        <button type="button" class="primary" @click="emit('view', run.path!, run.title)">
          {{ t("home.active.view") }}
        </button>
      </div>
    </section>

    <!-- 「進行中」與「模板」是兩區：上面是正在發生的事，下面是可以按的模板 -->
    <div v-if="home" class="section-head">
      <h2>{{ t("home.templates") }}</h2>
      <button v-if="home.profiles.length > 0" type="button" @click="createNew">{{ t("home.createNew") }}</button>
    </div>

    <div v-if="home && home.profiles.length === 0" class="panel empty">
      <h2>{{ t("home.empty.title") }}</h2>
      <p>{{ t("home.empty.body") }}</p>
      <div class="actions">
        <button type="button" class="primary" @click="createNew">{{ t("home.createNew") }}</button>
      </div>
      <!-- 加入現有的模板檔是少用的動作，平常放在設定頁；只有第一次（還沒有模板）時在這裡也給 -->
      <p class="muted">
        {{ t("home.empty.haveFile") }}
        <button type="button" class="link" @click="addExisting">{{ t("home.addExisting") }}</button>
      </p>
    </div>

    <ul v-if="home" class="cards">
      <li v-for="c in home.profiles" :key="c.path" class="panel card">
        <header class="card__head">
          <h2>
            {{ c.title }}
            <span v-if="c.external" class="tag">{{ t("home.card.external") }}</span>
          </h2>
          <template v-if="!c.error">
            <span v-if="c.busy" class="status busy">● {{ t("home.card.busy") }}</span>
            <span v-else :class="c.destinationConnected ? 'ok' : 'warn'" class="status">
              {{ c.destinationConnected ? "●" : "○" }}
              {{ c.destinationConnected ? t("home.card.connected") : t("home.card.notConnected") }}
            </span>
            <button
              type="button"
              class="primary"
              :disabled="home.running || isActive() || c.busy"
              @click="emit('start', c.path, c.title)"
            >
              {{ c.interrupted || c.last?.outcome === "cancelled" ? t("home.card.resume") : t("home.card.start") }}
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
          <p v-if="c.busy && run.path !== c.path" class="notice inline">{{ t("home.card.busyElsewhere") }}</p>
          <p v-else-if="!c.busy && (c.interrupted || c.last?.outcome === 'cancelled')" class="notice inline">
            {{ t("home.card.interrupted") }}
          </p>
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
            <dt>{{ t("home.card.schedule") }}</dt>
            <dd>
              <template v-if="c.schedule">{{ formatSchedule(c.schedule) }}</template>
              <span v-else class="muted">{{ t("home.card.noSchedule") }}</span>
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
.busy {
  color: var(--accent);
}
.inline {
  margin: 0 20px;
}
.active {
  margin-bottom: 14px;
  border: 2px solid var(--accent);
}
.active--waiting {
  border-color: var(--warn);
}
.active h2 {
  margin: 0 0 6px;
  font-size: 1rem;
  color: var(--muted);
}
.active {
  margin-bottom: 24px;
}
.active__body {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 20px;
  align-items: center;
  justify-content: space-between;
}
.active__body > div {
  flex: 1;
  min-width: 240px;
}
.active p {
  margin: 0 0 4px;
}
.active progress {
  width: 100%;
  height: 10px;
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
  display: grid;
  gap: 12px;
}
.empty h2,
.empty p {
  margin: 0;
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin: 4px 0 10px;
}
.section-head h2 {
  margin: 0;
  font-size: 1rem;
  color: var(--muted);
}
</style>
