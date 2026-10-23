<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Issue, type VersionStamp } from "../api";
import { t } from "../i18n/zh-TW";
import { formatBytes, formatNumber, formatStamp } from "../format";
import IssueList from "../components/IssueList.vue";

const props = defineProps<{ path: string; title: string; destination: string }>();
const emit = defineEmits<{ done: []; restore: [stamp: string] }>();

const stamps = ref<VersionStamp[] | null>(null);
const error = ref<Issue | null>(null);
const note = ref("");

async function refresh() {
  try {
    stamps.value = await api.versions(props.path);
  } catch (e) {
    error.value = asIssue(e);
  }
}

// 刪除前寫出完整路徑、檔案數、大小；按鈕上也寫數量（ui-workflows.md「舊版本」）。沒有「全部刪除」。
async function remove(s: VersionStamp) {
  const files = formatNumber(s.files);
  const size = formatBytes(s.bytes);
  const full = `${props.destination}\\_versions\\${s.name}`;
  const yes = await ask(t("versions.confirmBody", { path: full, files, size }), {
    title: t("versions.confirmTitle"),
    kind: "warning",
    okLabel: t("versions.confirmButton", { files, size }),
    cancelLabel: t("versions.keep"),
  });
  if (!yes) return;
  try {
    const gone = await api.deleteVersion(props.path, s.name);
    note.value = t("versions.deleted", { files: formatNumber(gone.files), size: formatBytes(gone.bytes) });
    error.value = null;
  } catch (e) {
    error.value = asIssue(e);
  }
  await refresh();
}

onMounted(refresh);
</script>

<template>
  <section class="panel">
    <h2>{{ t("versions.title", { title }) }}</h2>
    <p class="muted">{{ t("versions.help") }}</p>
    <IssueList v-if="error" :issues="[error]" />
    <p v-if="note" class="ok">✔ {{ note }}</p>
    <p v-if="stamps && stamps.length === 0" class="muted">{{ t("versions.empty") }}</p>
    <ul v-if="stamps" class="list">
      <li v-for="s in stamps" :key="s.name">
        <span>{{ t("versions.stamp", { when: formatStamp(s.name), files: formatNumber(s.files), size: formatBytes(s.bytes) }) }}</span>
        <span class="list__actions">
          <button type="button" @click="emit('restore', s.name)">{{ t("versions.restore") }}</button>
          <button type="button" @click="api.openFolder(path, 'version', s.name)">{{ t("versions.open") }}</button>
          <button type="button" class="danger" @click="remove(s)">{{ t("versions.delete") }}</button>
        </span>
      </li>
    </ul>
    <div class="actions">
      <button type="button" @click="emit('done')">{{ t("app.back") }}</button>
    </div>
  </section>
</template>

<style scoped>
.list {
  list-style: none;
  padding: 0;
  display: grid;
  gap: 8px;
}
.list li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid var(--border);
  padding-bottom: 8px;
}
.list__actions {
  display: flex;
  gap: 8px;
}
</style>
