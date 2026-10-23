<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Issue, type ProfileForm } from "../api";
import { t } from "../i18n/zh-TW";
import { splitList, WEEKDAYS } from "../format";
import { unsaved } from "../unsaved";
import IssueList from "../components/IssueList.vue";

// 同一個畫面給「編輯」與「建立新模板」用。建立時 `path` 是模板資料夾：
// 檔名由後端依名稱產生（library::new_path），建好之後才知道完整路徑。
const props = defineProps<{ path: string; creating: boolean }>();
const emit = defineEmits<{ done: [] }>();

const form = ref<ProfileForm>({
  title: "",
  destination: "",
  roots: [],
  excludeDirNames: ["node_modules", ".venv", "venv", "__pycache__", ".pytest_cache", "target"],
  excludeGlobs: [],
  blockLongPaths: false,
  verifyAfterCopy: true,
  alwaysSkipLocked: [],
  schedule: null,
});
/** 排程的編輯狀態：關掉時保留時間與星期，再打開不必重填。 */
const scheduleOn = ref(false);
const scheduleTime = ref("21:00");
const scheduleDays = ref<string[]>([]);
const excludeDirs = ref("");
const excludeGlobs = ref("");
const alwaysSkip = ref("");
const error = ref<Issue | null>(null);
const note = ref("");
const busy = ref(false);

/** 畫面上的值整理成要寫回的形狀；「有沒有改過」也拿它跟載入時比。 */
function current(): ProfileForm {
  return {
    ...form.value,
    title: form.value.title.trim(),
    destination: form.value.destination.trim(),
    roots: form.value.roots.map((r) => ({ path: r.path, name: r.name?.trim() ? r.name.trim() : null })),
    excludeDirNames: splitList(excludeDirs.value),
    excludeGlobs: splitList(excludeGlobs.value),
    alwaysSkipLocked: splitList(alwaysSkip.value),
    schedule: scheduleOn.value
      ? { time: scheduleTime.value, days: WEEKDAYS.map((d) => d.key).filter((k) => scheduleDays.value.includes(k)) }
      : null,
  };
}

// 載入完成那一刻的樣子。沒載入完之前不算「改過」。
const snapshot = ref<string | null>(null);
const dirty = computed(() => snapshot.value !== null && JSON.stringify(current()) !== snapshot.value);
watch(dirty, (d) => {
  unsaved.dirty = d;
  if (d) note.value = "";
});

onMounted(async () => {
  if (!props.creating) {
    try {
      form.value = await api.getProfile(props.path);
    } catch (e) {
      error.value = asIssue(e);
    }
  }
  excludeDirs.value = form.value.excludeDirNames.join(", ");
  excludeGlobs.value = form.value.excludeGlobs.join(", ");
  alwaysSkip.value = form.value.alwaysSkipLocked.join(", ");
  if (form.value.schedule) {
    scheduleOn.value = true;
    scheduleTime.value = form.value.schedule.time;
    scheduleDays.value = [...form.value.schedule.days];
  }
  snapshot.value = JSON.stringify(current());
  unsaved.save = save;
});

onUnmounted(() => {
  unsaved.dirty = false;
  unsaved.save = null;
});

const problem = computed(() => {
  if (!form.value.title.trim()) return t("edit.needName");
  if (form.value.roots.length === 0) return t("edit.needSource");
  if (!form.value.destination.trim()) return t("edit.needDestination");
  return "";
});

async function pickFolder(): Promise<string | null> {
  const p = await open({ directory: true, multiple: false });
  return typeof p === "string" ? p : null;
}

async function addSource() {
  const p = await pickFolder();
  if (p && !form.value.roots.some((r) => r.path.toLowerCase() === p.toLowerCase())) {
    form.value.roots.push({ path: p, name: null });
  }
}

async function pickDestination() {
  const p = await pickFolder();
  if (p) form.value.destination = p;
}

/** 存檔；成功回 true。「有未儲存的修改」的「儲存並離開」也呼叫這一個。 */
async function save(): Promise<boolean> {
  if (problem.value) return false;
  busy.value = true;
  error.value = null;
  note.value = "";
  const f = current();
  try {
    if (props.creating) {
      const created = await api.createProfile(f.title, f.destination, f.roots.map((r) => r.path));
      // 建立只寫最基本的欄位；其餘（別名、排除、進階）緊接著用一般的「儲存」寫進去。
      await api.saveProfile(created, f);
    } else {
      await api.saveProfile(props.path, f);
      note.value = t("edit.saved");
    }
    snapshot.value = JSON.stringify(f);
    unsaved.dirty = false;
    return true;
  } catch (e) {
    error.value = asIssue(e);
    return false;
  } finally {
    busy.value = false;
  }
}

async function submit() {
  if ((await save()) && props.creating) emit("done");
}

async function resetDestination() {
  try {
    await api.resetDestination(props.path);
    note.value = t("edit.saved");
  } catch (e) {
    error.value = asIssue(e);
  }
}
</script>

<template>
  <form class="edit" @submit.prevent="submit">
    <div class="panel edit__body">
      <h2>{{ creating ? t("edit.createTitle") : t("edit.title") }}</h2>
      <p class="muted path">{{ creating ? t("edit.willSaveIn", { folder: path }) : path }}</p>
      <IssueList v-if="error" :issues="[error]" />

      <section class="group">
        <h3>{{ t("edit.section.basic") }}</h3>
        <label class="field">
          <span>{{ t("edit.name") }}</span>
          <input v-model="form.title" type="text" required />
        </label>
      </section>

      <section class="group">
        <h3>{{ t("edit.sources") }}</h3>
        <p v-if="form.roots.length === 0" class="muted">{{ t("edit.noSource") }}</p>
        <ul class="roots">
          <li v-for="(r, i) in form.roots" :key="r.path">
            <span class="path roots__path">{{ r.path }}</span>
            <input v-model="r.name" type="text" :placeholder="t('edit.sourceAlias')" :aria-label="t('edit.sourceAlias')" />
            <button type="button" class="link" @click="form.roots.splice(i, 1)">{{ t("edit.removeSource") }}</button>
          </li>
        </ul>
        <div>
          <button type="button" @click="addSource">{{ t("edit.addSource") }}</button>
        </div>
      </section>

      <section class="group">
        <h3>{{ t("edit.section.exclude") }}</h3>
        <label class="field">
          <span>{{ t("edit.excludeDirs") }}</span>
          <input v-model="excludeDirs" type="text" />
        </label>
        <label class="field">
          <span>{{ t("edit.excludeGlobs") }}</span>
          <input v-model="excludeGlobs" type="text" />
        </label>
      </section>

      <section class="group">
        <h3>{{ t("edit.section.destination") }}</h3>
        <div class="field">
          <span>{{ t("edit.destination") }}</span>
          <div class="row">
            <input v-model="form.destination" type="text" class="path" />
            <button type="button" @click="pickDestination">{{ t("edit.pickDestination") }}</button>
          </div>
          <span class="muted hint">{{ t("edit.destinationHelp") }}</span>
        </div>
      </section>

      <section class="group">
        <h3>{{ t("edit.section.schedule") }}</h3>
        <label class="check"><input v-model="scheduleOn" type="checkbox" /> {{ t("edit.schedule.enable") }}</label>
        <template v-if="scheduleOn">
          <label class="field schedule-time">
            <span>{{ t("edit.schedule.time") }}</span>
            <input v-model="scheduleTime" type="time" required />
          </label>
          <fieldset class="field days">
            <legend>{{ t("edit.schedule.days") }}</legend>
            <label v-for="d in WEEKDAYS" :key="d.key" class="check">
              <input v-model="scheduleDays" type="checkbox" :value="d.key" /> {{ t(d.label) }}
            </label>
          </fieldset>
        </template>
        <p class="muted hint">{{ t("edit.schedule.help") }}</p>
      </section>

      <details class="group">
        <summary>{{ t("edit.advanced") }}</summary>
        <label class="check"><input v-model="form.blockLongPaths" type="checkbox" /> {{ t("edit.blockLongPaths") }}</label>
        <label class="check"><input v-model="form.verifyAfterCopy" type="checkbox" /> {{ t("edit.verify") }}</label>
        <label class="field">
          <span>{{ t("edit.alwaysSkip") }}</span>
          <input v-model="alwaysSkip" type="text" />
        </label>
        <p class="muted">{{ t("edit.keepForever") }}</p>
        <template v-if="!creating">
          <div>
            <button type="button" @click="resetDestination">{{ t("edit.resetDestination") }}</button>
          </div>
          <p class="muted">{{ t("edit.resetDestinationHelp") }}</p>
        </template>
      </details>
    </div>

    <!-- 固定在視窗底部：不必捲到最下面才找得到儲存，也一直看得到「有沒有存」 -->
    <div class="savebar panel">
      <span v-if="problem" class="warn">{{ problem }}</span>
      <span v-else-if="dirty" class="warn">● {{ t("edit.unsaved") }}</span>
      <span v-else-if="note" class="ok">✔ {{ note }}</span>
      <span v-else class="muted">{{ t("edit.noChanges") }}</span>
      <div class="actions">
        <button type="submit" class="primary" :disabled="busy || !!problem || (!creating && !dirty)">
          {{ creating ? t("edit.create") : t("edit.save") }}
        </button>
        <button type="button" @click="emit('done')">{{ creating ? t("edit.cancel") : t("app.back") }}</button>
      </div>
    </div>
  </form>
</template>

<style scoped>
.edit {
  display: grid;
  gap: 0;
}
.edit__body {
  display: grid;
  gap: 18px;
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
}
.edit h2 {
  margin: 0;
}
.edit h3 {
  margin: 0 0 4px;
  font-size: 1rem;
}
.path {
  word-break: break-all;
  font-family: "Cascadia Mono", Consolas, "Microsoft JhengHei UI", monospace;
  font-size: 0.92rem;
}
.edit__body > .path {
  margin: -12px 0 0;
}
.group {
  display: grid;
  gap: 8px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}
.group p {
  margin: 0;
}
.group summary {
  font-weight: 600;
  cursor: pointer;
}
.field {
  display: grid;
  gap: 4px;
}
.hint {
  font-size: 0.85rem;
}
.roots {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  gap: 6px;
}
.roots:empty {
  display: none;
}
.roots li {
  display: grid;
  grid-template-columns: 1fr 220px auto;
  gap: 8px;
  align-items: center;
}
.row {
  display: flex;
  gap: 8px;
}
.row input {
  flex: 1;
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
}
.schedule-time input {
  max-width: 160px;
}
.days {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 16px;
  border: none;
  padding: 0;
  margin: 0;
}
.days legend {
  width: 100%;
  margin-bottom: 4px;
}
.savebar {
  position: sticky;
  bottom: 0;
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 8px 16px;
  border-top-left-radius: 0;
  border-top-right-radius: 0;
  border-top: 2px solid var(--border);
  padding-top: 12px;
  padding-bottom: 12px;
  box-shadow: 0 -6px 12px rgb(0 0 0 / 12%);
}
.savebar .actions {
  margin: 0;
}
</style>
