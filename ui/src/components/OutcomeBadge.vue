<script setup lang="ts">
import type { Outcome } from "../api";
import { t } from "../i18n/zh-TW";

defineProps<{ outcome: Outcome }>();

// 狀態同時用圖示與文字表示，不只靠顏色（ui-workflows.md 原則 6）。
const icon: Record<Outcome, string> = {
  completed: "✔",
  completedWithWarnings: "⚠",
  failed: "✖",
  cancelled: "⦸",
};
</script>

<template>
  <span class="badge" :class="`badge--${outcome}`">
    <span aria-hidden="true">{{ icon[outcome] }}</span>
    {{ t(`outcome.${outcome}`) }}
  </span>
</template>

<style scoped>
.badge {
  display: inline-flex;
  gap: 4px;
  align-items: center;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 0.85rem;
  border: 1px solid currentColor;
}
.badge--completed {
  color: var(--ok);
}
.badge--completedWithWarnings {
  color: var(--warn);
}
.badge--failed {
  color: var(--bad);
}
.badge--cancelled {
  color: var(--muted);
}
</style>
