<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { api, type LockAnswer } from "../api";
import { t } from "../i18n/zh-TW";
import { formatBytes, formatDuration, formatNumber, formatPercent, formatRemaining } from "../format";
import { run, startRun, cancelRun, STEPS as steps } from "../run";
import IssueList from "../components/IssueList.vue";
import OutcomeBadge from "../components/OutcomeBadge.vue";

// `attach`：回到一次已經在跑的備份（首頁「進行中」的「查看」），不另外開始一次。
const props = defineProps<{ path: string; title: string; attach: boolean }>();
const emit = defineEmits<{ done: [] }>();

const remember = ref(false);
const openLock = ref<string | null>(null);

const stepIndex = computed(() => steps.findIndex((s) => s.states.includes(run.state)));

onMounted(async () => {
  if (!props.attach) await startRun(props.path, props.title);
});

async function confirm(yes: boolean) {
  run.confirmation = null;
  await api.answerConfirmation(yes);
}

async function answerLocks(a: LockAnswer) {
  run.locked = null;
  await api.answerLocks(a === "skip" && remember.value ? "skipAndRemember" : a);
  remember.value = false;
}

const report = computed(() => run.finished?.report ?? null);
const attention = computed(() => report.value?.issues.filter((i) => i.level !== "info").length ?? 0);
const headline = computed(() => {
  const r = report.value;
  if (!r) return "";
  if (r.outcome === "completedWithWarnings") return t("result.completedWithWarnings", { count: attention.value });
  return t(`result.${r.outcome}`);
});
const stampOfReport = computed(() => run.finished?.report.runId);
</script>

<template>
  <section class="panel run">
    <h2>{{ report ? title : t("run.title", { title }) }}</h2>
    <IssueList v-if="run.startError" :issues="[run.startError]" />

    <template v-if="!report">
      <ol class="steps">
        <li v-for="(s, i) in steps" :key="s.label" :class="{ done: i < stepIndex, now: i === stepIndex }">
          <span aria-hidden="true">{{ i < stepIndex ? "✔" : i === stepIndex ? "●" : "○" }}</span>
          {{ t(s.label) }}
        </li>
      </ol>

      <div v-if="run.progress && run.state === 'transferring'" class="progress">
        <progress :max="Math.max(run.progress.bytesTotal, 1)" :value="run.progress.bytesDone"></progress>
        <p>
          {{
            t("run.progress.files", {
              done: formatNumber(run.progress.filesDone),
              total: formatNumber(run.progress.filesTotal),
            })
          }}
          ·
          {{
            t("run.progress.bytes", {
              done: formatBytes(run.progress.bytesDone),
              total: formatBytes(run.progress.bytesTotal),
            })
          }}
          ·
          <strong>{{ formatRemaining(run.eta) }}</strong>
        </p>
        <p v-if="run.progress.current" class="muted current" :title="run.progress.current">
          {{ run.progress.current }}
        </p>
      </div>
      <p v-else-if="run.progress && run.state === 'scanning'" class="muted">
        {{ t("run.progress.scanned", { count: formatNumber(run.progress.filesDone) }) }}
      </p>
      <div v-else-if="run.progress && run.state === 'checkingLocks'" class="progress">
        <progress :max="Math.max(run.progress.filesTotal, 1)" :value="run.progress.filesDone"></progress>
        <p class="muted">
          {{
            t("run.progress.checked", {
              done: formatNumber(run.progress.filesDone),
              total: formatNumber(run.progress.filesTotal),
            })
          }}
        </p>
      </div>

      <!-- 需確認（state-machines.md §1） -->
      <div v-if="run.confirmation?.kind === 'massVersioning'" class="ask" role="alertdialog">
        <h3>⚠ {{ t("run.mass.title") }}</h3>
        <ul>
          <li v-for="m in run.confirmation.items" :key="m.root">
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
      <div v-if="run.confirmation?.kind === 'adoptDestination'" class="ask" role="alertdialog">
        <h3>{{ t("run.adopt.title") }}</h3>
        <p>{{ t("run.adopt.body", { path: run.confirmation.destination }) }}</p>
        <div class="actions">
          <button type="button" @click="confirm(false)">{{ t("run.decline") }}</button>
          <button type="button" class="primary" @click="confirm(true)">{{ t("run.adopt.confirm") }}</button>
        </div>
      </div>

      <!-- 需處理鎖定檔（ADR-0009） -->
      <div v-if="run.locked" class="ask" role="alertdialog">
        <h3>⚠ {{ t("run.locks.title") }}</h3>
        <p class="muted">{{ t("run.locks.help") }}</p>
        <ul class="locks">
          <li v-for="g in run.locked" :key="g.app">
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
      <div v-if="!run.confirmation && !run.locked" class="actions">
        <button type="button" :disabled="run.cancelling" @click="cancelRun">
          {{ run.cancelling ? t("run.cancelPending") : t("run.cancel") }}
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
        <button v-if="report.outcome === 'failed' || report.outcome === 'cancelled'" type="button" @click="startRun(path, title)">
          {{ t("result.retry") }}
        </button>
        <!-- 有檔案這次沒備份到（被佔用、一直在變動）：再跑一次就只處理它們，其餘的都會被算成「沒有變更」 -->
        <button
          v-else-if="report.counts.skipped > 0"
          type="button"
          :title="t('result.retrySkippedHelp')"
          @click="startRun(path, title)"
        >
          {{ t("result.retrySkipped", { count: formatNumber(report.counts.skipped) }) }}
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
/* grid 子元素預設不會比內容窄：一條很長的路徑會把整個面板撐出視窗。 */
.run > *,
.progress > * {
  min-width: 0;
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
