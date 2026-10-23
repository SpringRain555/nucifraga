<script setup lang="ts">
import { onMounted, ref } from "vue";
import { api, asIssue, type Issue, type Report } from "../api";
import { t } from "../i18n/zh-TW";
import { formatBytes, formatDate, formatNumber } from "../format";
import IssueList from "../components/IssueList.vue";
import OutcomeBadge from "../components/OutcomeBadge.vue";

const props = defineProps<{ path: string; title: string }>();
const emit = defineEmits<{ done: [] }>();

const reports = ref<Report[] | null>(null);
const error = ref<Issue | null>(null);
const selected = ref<Report | null>(null);

onMounted(async () => {
  try {
    reports.value = await api.history(props.path);
  } catch (e) {
    error.value = asIssue(e);
  }
});
</script>

<template>
  <section class="panel">
    <h2>{{ t("history.title", { title }) }}</h2>
    <IssueList v-if="error" :issues="[error]" />
    <p v-if="reports && reports.length === 0" class="muted">{{ t("history.empty") }}</p>
    <ul v-if="reports" class="list">
      <li v-for="r in reports" :key="r.runId">
        <OutcomeBadge :outcome="r.outcome" />
        <span>{{ formatDate(r.finishedAt) }}</span>
        <span class="muted">
          {{ t("result.copied") }} {{ formatNumber(r.counts.copied) }} · {{ t("result.updated") }} {{ formatNumber(r.counts.updated) }} ·
          {{ t("result.versioned") }} {{ formatNumber(r.counts.versioned) }} · {{ formatBytes(r.bytesCopied) }}
        </span>
        <button type="button" class="link" @click="selected = selected?.runId === r.runId ? null : r">{{ t("history.open") }}</button>
        <div v-if="selected?.runId === r.runId" class="detail">
          <IssueList v-if="r.issues.length" :issues="r.issues" />
          <button type="button" @click="api.openFolder(path, 'report', r.runId)">{{ t("result.openReport") }}</button>
        </div>
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
.list > li {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  border-bottom: 1px solid var(--border);
  padding-bottom: 8px;
}
.detail {
  width: 100%;
  display: grid;
  gap: 8px;
}
</style>
