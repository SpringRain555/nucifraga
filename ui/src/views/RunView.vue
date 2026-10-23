<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  api,
  asIssue,
  type Confirmation,
  type Finished,
  type Issue,
  type LockAnswer,
  type LockGroup,
  type Progress,
  type RunState,
} from "../api";
import { t, type MessageKey } from "../i18n/zh-TW";
import { formatBytes, formatDuration, formatNumber, formatPercent } from "../format";
import IssueList from "../components/IssueList.vue";
import OutcomeBadge from "../components/OutcomeBadge.vue";

const props = defineProps<{ path: string; title: string }>();
const emit = defineEmits<{ done: [] }>();

const state = ref<RunState>("idle");
const progress = ref<Progress | null>(null);
const confirmation = ref<Confirmation | null>(null);
const locked = ref<LockGroup[] | null>(null);
const remember = ref(false);
const finished = ref<Finished | null>(null);
const cancelling = ref(false);
const startError = ref<Issue | null>(null);
const openLock = ref<string | null>(null);

const steps: { states: RunState[]; label: MessageKey }[] = [
  { states: ["preflight", "awaitingConfirmation"], label: "run.step.preflight" },
  { states: ["scanning"], label: "run.step.scanning" },
  { states: ["planning"], label: "run.step.planning" },
  { states: ["checkingLocks", "awaitingLockDecision"], label: "run.step.checkingLocks" },
  { states: ["transferring"], label: "run.step.transferring" },
  { states: ["finalizing"], label: "run.step.finalizing" },
];
const stepIndex = computed(() => steps.findIndex((s) => s.states.includes(state.value)));

const unlisten: UnlistenFn[] = [];

onMounted(async () => {
  unlisten.push(
    await api.onRunEvent((e) => {
      switch (e.kind) {
        case "state":
          state.value = e.data;
          if (e.data !== "awaitingConfirmation") confirmation.value = null;
          if (e.data !== "awaitingLockDecision") locked.value = null;
          break;
        case "progress":
          progress.value = e.data;
          break;
        case "confirmation":
          confirmation.value = e.data;
          break;
        case "locked":
          locked.value = e.data;
          remember.value = false;
          break;
        case "issue":
          break; // 結果畫面一次列出全部，不在進行中逐則跳出
      }
    }),
  );
  unlisten.push(await api.onRunFinished((f) => (finished.value = f)));
  await start();
});

async function start() {
  state.value = "idle";
  progress.value = null;
  finished.value = null;
  cancelling.value = false;
  startError.value = null;
  try {
    await api.startRun(props.path);
  } catch (e) {
    startError.value = asIssue(e);
  }
}
onUnmounted(() => unlisten.forEach((u) => u()));

async function confirm(yes: boolean) {
  confirmation.value = null;
  await api.answerConfirmation(yes);
}

async function answerLocks(a: LockAnswer) {
  locked.value = null;
  await api.answerLocks(a === "skip" && remember.value ? "skipAndRemember" : a);
}

async function cancel() {
  cancelling.value = true;
  await api.cancelRun(false);
}

const report = computed(() => finished.value?.report ?? null);
const attention = computed(() => report.value?.issues.filter((i) => i.level !== "info").length ?? 0);
const headline = computed(() => {
  const r = report.value;
  if (!r) return "";
  if (r.outcome === "completedWithWarnings") return t("result.completedWithWarnings", { count: attention.value });
  return t(`result.${r.outcome}`);
});
const stampOfReport = computed(() => finished.value?.report.runId);
</script>

<template>
  <section class="panel run">
    <h2>{{ report ? title : t("run.title", { title }) }}</h2>
    <IssueList v-if="startError" :issues="[startError]" />

    <template v-if="!report">
      <ol class="steps">
        <li v-for="(s, i) in steps" :key="s.label" :class="{ done: i < stepIndex, now: i === stepIndex }">
          <span aria-hidden="true">{{ i < stepIndex ? "✔" : i === stepIndex ? "●" : "○" }}</span>
          {{ t(s.label) }}
        </li>
      </ol>

      <div v-if="progress && state === 'transferring'" class="progress">
        <progress :max="Math.max(progress.bytesTotal, 1)" :value="progress.bytesDone"></progress>
        <p>
          {{ t("run.progress.files", { done: formatNumber(progress.filesDone), total: formatNumber(progress.filesTotal) }) }}
          ·
          {{ t("run.progress.bytes", { done: formatBytes(progress.bytesDone), total: formatBytes(progress.bytesTotal) }) }}
        </p>
        <p v-if="progress.current" class="muted current" :title="progress.current">{{ progress.current }}</p>
      </div>
      <p v-else-if="progress && state === 'scanning'" class="muted">
        {{ t("run.progress.scanned", { count: formatNumber(progress.filesDone) }) }}
      </p>

      <!-- 需確認（state-machines.md §1） -->
      <div v-if="confirmation?.kind === 'massVersioning'" class="ask" role="alertdialog">
        <h3>⚠ {{ t("run.mass.title") }}</h3>
        <ul>
          <li v-for="m in confirmation.items" :key="m.root">
            {{ t("run.mass.item", { root: m.root, count: formatNumber(m.count), ratio: formatPercent(m.ratio) }) }}
            <ul class="sample">
              <li v-for="s in m.sample" :key="s">{{ s }}</li>
            </ul>
          </li>
        </ul>
        <p class="muted">{{ t("run.mass.help") }}</p>
        <div class="actions">
          <button type="button" @click="confirm(false)">{{ t("run.decline") }}</button>
          <button type="button" class="primary" @click="confirm(true)">{{ t("run.mass.confirm") }}</button>
        </div>
      </div>
      <div v-if="confirmation?.kind === 'adoptDestination'" class="ask" role="alertdialog">
        <h3>{{ t("run.adopt.title") }}</h3>
        <p>{{ t("run.adopt.body", { path: confirmation.destination }) }}</p>
        <div class="actions">
          <button type="button" @click="confirm(false)">{{ t("run.decline") }}</button>
          <button type="button" class="primary" @click="confirm(true)">{{ t("run.adopt.confirm") }}</button>
        </div>
      </div>

      <!-- 需處理鎖定檔（ADR-0009） -->
      <div v-if="locked" class="ask" role="alertdialog">
        <h3>⚠ {{ t("run.locks.title") }}</h3>
        <p class="muted">{{ t("run.locks.help") }}</p>
        <ul class="locks">
          <li v-for="g in locked" :key="g.app">
            <button type="button" class="link" @click="openLock = openLock === g.app ? null : g.app">
              {{ t("run.locks.group", { app: g.app, count: g.files.length }) }}
            </button>
            <ul v-if="openLock === g.app" class="sample">
              <li v-for="f in g.files" :key="f.root + f.rel">{{ f.root }}\{{ f.rel.replaceAll("/", "\\") }}</li>
            </ul>
          </li>
        </ul>
        <label class="check"><input v-model="remember" type="checkbox" /> {{ t("run.locks.remember") }}</label>
        <div class="actions">
          <button type="button" @click="answerLocks('cancel')">{{ t("run.decline") }}</button>
          <button type="button" @click="answerLocks('skip')">{{ t("run.locks.skip") }}</button>
          <button type="button" class="primary" @click="answerLocks('retry')">{{ t("run.locks.retry") }}</button>
        </div>
      </div>

      <!-- 停下來問的時候，問題本身已經有「取消這次備份」，這裡不再放第二個 -->
      <div v-if="!confirmation && !locked" class="actions">
        <button type="button" :disabled="cancelling" @click="cancel">
          {{ cancelling ? t("run.cancelPending") : t("run.cancel") }}
        </button>
      </div>
    </template>

    <!-- 結果 -->
    <template v-else>
      <p class="headline">
        <OutcomeBadge :outcome="report.outcome" />
        <strong>{{ headline }}</strong>
      </p>
      <dl class="numbers">
        <div><dt>{{ t("result.copied") }}</dt><dd>{{ formatNumber(report.counts.copied) }}</dd></div>
        <div><dt>{{ t("result.updated") }}</dt><dd>{{ formatNumber(report.counts.updated) }}</dd></div>
        <div><dt>{{ t("result.versioned") }}</dt><dd>{{ formatNumber(report.counts.versioned) }}</dd></div>
        <div><dt>{{ t("result.unchanged") }}</dt><dd>{{ formatNumber(report.counts.unchanged + report.counts.verified) }}</dd></div>
        <div><dt>{{ t("result.skipped") }}</dt><dd>{{ formatNumber(report.counts.skipped) }}</dd></div>
        <div><dt>{{ t("result.written") }}</dt><dd>{{ formatBytes(report.bytesCopied) }}</dd></div>
        <div><dt>{{ t("result.duration") }}</dt><dd>{{ formatDuration(report.startedAt, report.finishedAt) }}</dd></div>
      </dl>
      <template v-if="report.issues.length">
        <h3>{{ t("result.issues") }}</h3>
        <IssueList :issues="report.issues" />
      </template>
      <div class="actions">
        <button type="button" class="primary" @click="emit('done')">{{ t("result.done") }}</button>
        <!-- 失敗或取消後：接上備份碟、關掉程式之後直接再跑一次；已經傳完的會被跳過（state-machines.md「中斷之後」） -->
        <button v-if="report.outcome === 'failed' || report.outcome === 'cancelled'" type="button" @click="start">
          {{ t("result.retry") }}
        </button>
        <button v-if="report.outcome !== 'failed'" type="button" @click="api.openFolder(path, 'current')">
          {{ t("result.openCurrent") }}
        </button>
        <button v-if="stampOfReport && report.destination" type="button" @click="api.openFolder(path, 'report', stampOfReport)">
          {{ t("result.openReport") }}
        </button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.run {
  display: grid;
  gap: 14px;
}
.run h2 {
  margin: 0;
}
.steps {
  list-style: none;
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  padding: 0;
  margin: 0;
  color: var(--muted);
}
.steps .done {
  color: var(--ok);
}
.steps .now {
  color: var(--text);
  font-weight: 600;
}
.progress progress {
  width: 100%;
  height: 14px;
}
.current {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  direction: rtl;
  text-align: left;
}
.ask {
  border: 2px solid var(--warn);
  border-radius: 8px;
  padding: 12px 16px;
}
.ask h3 {
  margin-top: 0;
}
.sample {
  font-size: 0.85rem;
  color: var(--muted);
  max-height: 160px;
  overflow: auto;
  word-break: break-all;
}
.locks {
  padding-left: 18px;
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
}
.headline {
  display: flex;
  gap: 10px;
  align-items: center;
  font-size: 1.2rem;
  margin: 0;
}
.numbers {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 8px;
  margin: 0;
}
.numbers div {
  background: var(--bg);
  border-radius: 6px;
  padding: 8px;
}
.numbers dt {
  color: var(--muted);
  font-size: 0.85rem;
}
.numbers dd {
  margin: 0;
  font-size: 1.2rem;
  font-weight: 600;
}
</style>
