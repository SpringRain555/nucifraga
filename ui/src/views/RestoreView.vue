<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  api,
  asIssue,
  type Issue,
  type OnConflict,
  type RestoreEntry,
  type RestoreOutcome,
  type RestorePreview,
  type RestoreProgress,
  type RestoreSource,
  type RestoreTarget,
  type VersionStamp,
} from "../api";
import { t } from "../i18n/zh-TW";
import { formatBytes, formatNumber, formatStamp } from "../format";
import IssueList from "../components/IssueList.vue";

// 還原（ADR-0011）：選時間點 → 勾檔案 → 選放哪 → 檢查 → 還原。絕不覆蓋任何檔案。
const props = defineProps<{ path: string; title: string; stamp: string | null }>();
const emit = defineEmits<{ done: [] }>();

type Phase = "choose" | "running" | "done";
const phase = ref<Phase>("choose");
const error = ref<Issue | null>(null);

const stamps = ref<VersionStamp[]>([]);
/** "" ＝ 目前版本；其餘是 `_versions` 的時間點。 */
const sourceKey = ref(props.stamp ?? "");
const source = computed<RestoreSource>(() =>
  sourceKey.value ? { kind: "version", stamp: sourceKey.value } : { kind: "current" },
);

const crumbs = ref<string[]>([]);
const entries = ref<RestoreEntry[]>([]);
const selected = ref<string[]>([]);
const rel = computed(() => crumbs.value.join("/"));
const full = (name: string) => (rel.value ? `${rel.value}/${name}` : name);

const targetKind = ref<"newFolder" | "original">("newFolder");
const parent = ref("");
const target = computed<RestoreTarget>(() =>
  targetKind.value === "newFolder" ? { kind: "newFolder", folder: parent.value } : { kind: "original" },
);
const onConflict = ref<OnConflict>("skip");
const preview = ref<RestorePreview | null>(null);
const progress = ref<RestoreProgress | null>(null);
const outcome = ref<RestoreOutcome | null>(null);
const unlisten: UnlistenFn[] = [];

async function load() {
  try {
    error.value = null;
    entries.value = await api.restoreBrowse(props.path, source.value, rel.value);
  } catch (e) {
    entries.value = [];
    error.value = asIssue(e);
  }
}

onMounted(async () => {
  try {
    stamps.value = await api.versions(props.path);
    parent.value = (await api.diagnostics()).desktop;
  } catch (e) {
    error.value = asIssue(e);
  }
  unlisten.push(await api.onRestoreProgress((p) => (progress.value = p)));
  unlisten.push(
    await api.onRestoreFinished((o) => {
      outcome.value = o;
      phase.value = "done";
    }),
  );
  await load();
});
onUnmounted(() => unlisten.forEach((u) => u()));

// 換時間點：回到最上層、清掉勾選（同一個路徑在不同時間點不一定存在）。
watch(sourceKey, async () => {
  crumbs.value = [];
  selected.value = [];
  preview.value = null;
  await load();
});
watch([selected, targetKind, parent], () => (preview.value = null), { deep: true });

async function enter(name: string) {
  crumbs.value.push(name);
  await load();
}
async function goUp(level: number) {
  crumbs.value = crumbs.value.slice(0, level);
  await load();
}
function toggle(name: string) {
  const f = full(name);
  selected.value = selected.value.includes(f) ? selected.value.filter((s) => s !== f) : [...selected.value, f];
}

async function pickParent() {
  const p = await open({ directory: true, multiple: false, defaultPath: parent.value });
  if (typeof p === "string") parent.value = p;
}

async function check() {
  try {
    error.value = null;
    preview.value = await api.restorePreview(props.path, source.value, selected.value, target.value);
  } catch (e) {
    error.value = asIssue(e);
  }
}

async function start() {
  try {
    error.value = null;
    progress.value = null;
    phase.value = "running";
    await api.restoreStart(props.path, source.value, selected.value, target.value, onConflict.value);
  } catch (e) {
    phase.value = "choose";
    error.value = asIssue(e);
  }
}
</script>

<template>
  <section class="panel restore">
    <h2>{{ t("restore.title", { title }) }}</h2>
    <IssueList v-if="error" :issues="[error]" />

    <template v-if="phase === 'choose'">
      <section class="group">
        <h3>{{ t("restore.step.source") }}</h3>
        <select v-model="sourceKey" class="source">
          <option value="">{{ t("restore.current") }}</option>
          <option v-for="s in stamps" :key="s.name" :value="s.name">
            {{ t("restore.version", { when: formatStamp(s.name), files: formatNumber(s.files) }) }}
          </option>
        </select>
        <p class="muted hint">{{ sourceKey ? t("restore.versionHelp") : t("restore.currentHelp") }}</p>
      </section>

      <section class="group">
        <h3>{{ t("restore.step.pick") }}</h3>
        <nav class="crumbs">
          <button type="button" class="link" @click="goUp(0)">{{ t("restore.top") }}</button>
          <template v-for="(c, i) in crumbs" :key="i">
            <span class="muted">\</span>
            <button type="button" class="link" @click="goUp(i + 1)">{{ c }}</button>
          </template>
        </nav>
        <p v-if="entries.length === 0 && !error" class="muted">{{ t("restore.emptyFolder") }}</p>
        <ul class="entries">
          <li v-for="e in entries" :key="e.name">
            <label class="check">
              <input type="checkbox" :checked="selected.includes(full(e.name))" @change="toggle(e.name)" />
            </label>
            <button v-if="e.isDir" type="button" class="link name" @click="enter(e.name)">📁 {{ e.name }}</button>
            <span v-else class="name">{{ e.name }}</span>
            <span class="muted size">{{ e.isDir ? "" : formatBytes(e.size) }}</span>
          </li>
        </ul>
        <p class="muted">
          {{ t("restore.selected", { count: selected.length }) }}
          <button v-if="selected.length" type="button" class="link" @click="selected = []">{{ t("restore.clear") }}</button>
        </p>
      </section>

      <section class="group">
        <h3>{{ t("restore.step.target") }}</h3>
        <label class="check">
          <input v-model="targetKind" type="radio" value="newFolder" /> {{ t("restore.toNewFolder") }}
        </label>
        <div v-if="targetKind === 'newFolder'" class="row">
          <span class="path">{{ parent }}</span>
          <button type="button" @click="pickParent">{{ t("restore.pickParent") }}</button>
        </div>
        <label class="check">
          <input v-model="targetKind" type="radio" value="original" /> {{ t("restore.toOriginal") }}
        </label>
        <p class="muted hint">{{ t("restore.neverOverwrite") }}</p>
      </section>

      <div class="actions">
        <button type="button" :disabled="selected.length === 0" @click="check">{{ t("restore.check") }}</button>
        <button type="button" @click="emit('done')">{{ t("app.back") }}</button>
      </div>

      <section v-if="preview" class="ask">
        <p>
          <strong>{{ t("restore.summary", { files: formatNumber(preview.files), size: formatBytes(preview.bytes) }) }}</strong>
        </p>
        <p v-if="preview.folder" class="path">{{ t("restore.into", { folder: preview.folder }) }}</p>
        <template v-if="preview.conflicts > 0">
          <p class="warn">⚠ {{ t("restore.conflicts", { count: formatNumber(preview.conflicts) }) }}</p>
          <ul class="sample">
            <li v-for="c in preview.conflictSample" :key="c">{{ c }}</li>
          </ul>
          <label class="check">
            <input v-model="onConflict" type="radio" value="skip" /> {{ t("restore.conflictSkip") }}
          </label>
          <label class="check">
            <input v-model="onConflict" type="radio" value="keepBoth" /> {{ t("restore.conflictKeepBoth") }}
          </label>
        </template>
        <div class="actions">
          <button type="button" class="primary" :disabled="preview.files === 0" @click="start">
            {{ t("restore.start", { files: formatNumber(preview.files) }) }}
          </button>
        </div>
      </section>
    </template>

    <template v-else-if="phase === 'running'">
      <progress :max="Math.max(progress?.bytesTotal ?? 1, 1)" :value="progress?.bytes ?? 0"></progress>
      <p>
        {{ t("restore.progress", { done: formatNumber(progress?.done ?? 0), total: formatNumber(progress?.total ?? 0) }) }}
      </p>
      <div class="actions">
        <button type="button" @click="api.restoreCancel()">{{ t("restore.cancel") }}</button>
      </div>
    </template>

    <template v-else-if="outcome">
      <p class="headline">
        <strong :class="outcome.issues.length ? 'warn' : 'ok'">
          {{ outcome.cancelled ? t("restore.doneCancelled") : outcome.issues.length ? t("restore.doneWarn") : t("restore.doneOk") }}
        </strong>
      </p>
      <p>
        {{
          t("restore.counts", {
            restored: formatNumber(outcome.restored),
            renamed: formatNumber(outcome.renamed),
            skipped: formatNumber(outcome.skipped),
            size: formatBytes(outcome.bytes),
          })
        }}
      </p>
      <IssueList v-if="outcome.issues.length" :issues="outcome.issues" />
      <div class="actions">
        <button v-if="outcome.folder" type="button" class="primary" @click="api.openRestoredFolder()">
          {{ t("restore.openFolder") }}
        </button>
        <button type="button" @click="emit('done')">{{ t("result.done") }}</button>
      </div>
    </template>
  </section>
</template>

<style scoped>
.restore {
  display: grid;
  gap: 16px;
}
.restore > * {
  min-width: 0;
}
.restore h2,
.restore h3,
.restore p {
  margin: 0;
}
.group {
  display: grid;
  gap: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--border);
}
.group h3 {
  font-size: 1rem;
}
.source {
  max-width: 420px;
  padding: 6px 8px;
  font: inherit;
  color: var(--text);
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 6px;
}
.crumbs {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  align-items: baseline;
}
.entries {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 360px;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 6px;
}
.entries li {
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: 8px;
  align-items: center;
  padding: 4px 10px;
  border-bottom: 1px solid var(--border);
}
.entries li:last-child {
  border-bottom: none;
}
.name {
  text-align: left;
  word-break: break-all;
}
.size {
  font-size: 0.85rem;
  white-space: nowrap;
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
}
.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  align-items: center;
  padding-left: 24px;
}
.path {
  word-break: break-all;
  font-family: "Cascadia Mono", Consolas, "Microsoft JhengHei UI", monospace;
  font-size: 0.92rem;
}
.hint {
  font-size: 0.85rem;
}
.ask {
  display: grid;
  gap: 8px;
  border: 2px solid var(--accent);
  border-radius: 8px;
  padding: 12px 16px;
}
.sample {
  font-size: 0.85rem;
  color: var(--muted);
  max-height: 160px;
  overflow: auto;
  word-break: break-all;
  margin: 0;
}
progress {
  width: 100%;
  height: 14px;
}
.headline {
  font-size: 1.2rem;
}
</style>
