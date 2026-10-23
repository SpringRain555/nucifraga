<script setup lang="ts">
import { onMounted, ref } from "vue";
import { t } from "../i18n/zh-TW";

// 「有未儲存的修改」：換頁或關視窗時由 App 顯示（ui-workflows「模板編輯」）。
defineProps<{ quitting: boolean; saving: boolean }>();
const emit = defineEmits<{ save: []; discard: []; stay: [] }>();

// 焦點先放在「繼續編輯」：最不會丟東西的那一側（ui-workflows「無障礙與鍵盤」）。
const stayButton = ref<HTMLButtonElement | null>(null);
onMounted(() => stayButton.value?.focus());
</script>

<template>
  <div class="backdrop" @keydown.esc="emit('stay')">
    <div class="dialog panel" role="alertdialog" aria-modal="true" aria-labelledby="leave-title">
      <h2 id="leave-title">{{ t("leave.title") }}</h2>
      <p>{{ quitting ? t("leave.bodyQuit") : t("leave.body") }}</p>
      <div class="actions">
        <button type="button" class="primary" :disabled="saving" @click="emit('save')">
          {{ quitting ? t("leave.saveQuit") : t("leave.save") }}
        </button>
        <button type="button" :disabled="saving" @click="emit('discard')">
          {{ quitting ? t("leave.discardQuit") : t("leave.discard") }}
        </button>
        <button ref="stayButton" type="button" :disabled="saving" @click="emit('stay')">{{ t("leave.stay") }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  background: rgb(0 0 0 / 45%);
  display: grid;
  place-items: center;
  padding: 16px;
  z-index: 10;
}
.dialog {
  max-width: 480px;
  width: 100%;
  border: 2px solid var(--warn);
}
.dialog h2 {
  margin-top: 0;
  font-size: 1.2rem;
}
</style>
