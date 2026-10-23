<script setup lang="ts">
import { computed, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Issue } from "../api";
import { t } from "../i18n/zh-TW";
import IssueList from "../components/IssueList.vue";

// 同一個畫面給「第一次使用」與「更換模板資料夾」用（ui-workflows.md「模板資料夾」）。
const props = defineProps<{
  /** 現在的模板資料夾；null ＝ 第一次使用。 */
  current: string | null;
  suggested: string;
  /** 現在資料夾裡有幾份模板（更換時問要不要一起搬）。 */
  inFolder: number;
}>();
const emit = defineEmits<{ done: [] }>();

const first = computed(() => props.current === null);
const chosen = ref(props.current ?? props.suggested);
const moveExisting = ref(true);
const issues = ref<Issue[]>([]);
const busy = ref(false);
const changed = computed(() => chosen.value.toLowerCase() !== (props.current ?? "").toLowerCase());

async function pick() {
  const p = await open({ directory: true, multiple: false, defaultPath: chosen.value });
  if (typeof p === "string") chosen.value = p;
}

async function use() {
  busy.value = true;
  issues.value = [];
  try {
    const left = await api.setProfilesFolder(chosen.value, !first.value && props.inFolder > 0 && moveExisting.value);
    // 有沒搬成的就留在這個畫面讓使用者看完；全部順利就直接回首頁。
    if (left.length) issues.value = left;
    else emit("done");
  } catch (e) {
    issues.value = [asIssue(e)];
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section class="panel folder">
    <h2>{{ first ? t("folder.firstTitle") : t("folder.changeTitle") }}</h2>
    <p>{{ first ? t("folder.firstBody") : t("folder.changeBody") }}</p>
    <p v-if="!first" class="muted">{{ t("folder.current", { path: current ?? "" }) }}</p>

    <div class="chosen">
      <span class="muted">{{ t("folder.chosen") }}</span>
      <code>{{ chosen }}</code>
    </div>

    <label v-if="!first && changed && inFolder > 0" class="check">
      <input v-model="moveExisting" type="checkbox" /> {{ t("folder.move", { count: inFolder }) }}
    </label>

    <p class="muted">{{ t("folder.externalNote") }}</p>
    <IssueList v-if="issues.length" :issues="issues" />

    <div class="actions">
      <template v-if="issues.length && !busy">
        <button type="button" class="primary" @click="emit('done')">{{ t("app.back") }}</button>
      </template>
      <template v-else>
        <button type="button" class="primary" :disabled="busy || (!first && !changed)" @click="use">
          {{ t("folder.use") }}
        </button>
        <button type="button" :disabled="busy" @click="pick">{{ t("folder.pick") }}</button>
        <button v-if="!first" type="button" :disabled="busy" @click="emit('done')">{{ t("folder.cancel") }}</button>
      </template>
    </div>
  </section>
</template>

<style scoped>
.folder {
  display: grid;
  gap: 12px;
}
.folder h2,
.folder p {
  margin: 0;
}
.chosen {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: baseline;
}
.chosen code {
  word-break: break-all;
  font-size: 1.05rem;
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
}
</style>
