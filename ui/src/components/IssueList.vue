<script setup lang="ts">
import { computed, ref } from "vue";
import type { Issue } from "../api";
import { t } from "../i18n/zh-TW";

const props = defineProps<{ issues: Issue[] }>();

// 依錯誤碼分組：同一種問題只顯示一次訊息與建議動作，檔案收在「查看檔案」裡。
const groups = computed(() => {
  const order: Issue["level"][] = ["fatal", "error", "warning", "info"];
  const map = new Map<string, { first: Issue; files: string[] }>();
  for (const i of props.issues) {
    const g = map.get(i.code) ?? { first: i, files: [] };
    if (i.path) g.files.push(`${i.root ?? ""}\\${i.path.replaceAll("/", "\\")}`);
    map.set(i.code, g);
  }
  return [...map.values()].sort((a, b) => order.indexOf(a.first.level) - order.indexOf(b.first.level));
});

const open = ref<Set<string>>(new Set());
function toggle(code: string) {
  const s = new Set(open.value);
  if (s.has(code)) s.delete(code);
  else s.add(code);
  open.value = s;
}
</script>

<template>
  <ul class="issues">
    <li v-for="g in groups" :key="g.first.code" :class="`issue issue--${g.first.level}`">
      <div class="issue__head">
        <code>{{ g.first.code }}</code>
        <span>{{ g.first.message }}</span>
      </div>
      <div v-if="g.first.action" class="issue__action">{{ t("result.action") }}{{ g.first.action }}</div>
      <pre v-if="g.first.detail && g.files.length <= 1" class="issue__detail">{{ g.first.detail }}</pre>
      <template v-if="g.files.length > 1">
        <button type="button" class="link" @click="toggle(g.first.code)">
          {{ open.has(g.first.code) ? t("result.hideFiles") : t("result.showFiles", { count: g.files.length }) }}
        </button>
        <ul v-if="open.has(g.first.code)" class="issue__files">
          <li v-for="f in g.files" :key="f">{{ f }}</li>
        </ul>
      </template>
    </li>
  </ul>
</template>

<style scoped>
.issues {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 10px;
}
.issue {
  border-left: 4px solid var(--muted);
  background: var(--surface);
  padding: 8px 12px;
  border-radius: 4px;
}
.issue--fatal,
.issue--error {
  border-left-color: var(--bad);
}
.issue--warning {
  border-left-color: var(--warn);
}
.issue__head {
  display: flex;
  gap: 8px;
  align-items: baseline;
}
.issue__action {
  color: var(--muted);
  margin-top: 2px;
}
.issue__detail {
  white-space: pre-wrap;
  word-break: break-all;
  font-size: 0.8rem;
  color: var(--muted);
  margin: 6px 0 0;
}
.issue__files {
  font-size: 0.85rem;
  max-height: 200px;
  overflow: auto;
  word-break: break-all;
}
</style>
