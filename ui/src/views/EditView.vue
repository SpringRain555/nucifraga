<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { api, asIssue, type Issue, type ProfileForm } from "../api";
import { t } from "../i18n/zh-TW";
import { splitList } from "../format";
import IssueList from "../components/IssueList.vue";

// 同一個畫面給「編輯」與「建立新模板」用；建立時 `creating` 為 true，`path` 是使用者選的存檔位置。
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
});
const excludeDirs = ref("");
const excludeGlobs = ref("");
const alwaysSkip = ref("");
const error = ref<Issue | null>(null);
const note = ref("");
const busy = ref(false);

onMounted(async () => {
  if (!props.creating) {
    try {
      form.value = await api.getProfile(props.path);
    } catch (e) {
      error.value = asIssue(e);
    }
  } else {
    form.value.title = props.path.split(/[\\/]/).pop()?.replace(/\.json$/i, "") ?? "";
  }
  excludeDirs.value = form.value.excludeDirNames.join(", ");
  excludeGlobs.value = form.value.excludeGlobs.join(", ");
  alwaysSkip.value = form.value.alwaysSkipLocked.join(", ");
});

const problem = computed(() => {
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

async function submit() {
  if (problem.value) return;
  busy.value = true;
  error.value = null;
  note.value = "";
  const f: ProfileForm = {
    ...form.value,
    roots: form.value.roots.map((r) => ({ path: r.path, name: r.name?.trim() ? r.name.trim() : null })),
    excludeDirNames: splitList(excludeDirs.value),
    excludeGlobs: splitList(excludeGlobs.value),
    alwaysSkipLocked: splitList(alwaysSkip.value),
  };
  try {
    if (props.creating) {
      await api.createProfile(props.path, f.title, f.destination, f.roots.map((r) => r.path));
      // 建立只寫最基本的欄位；其餘（別名、排除、進階）緊接著用一般的「儲存」寫進去。
      await api.saveProfile(props.path, f);
      emit("done");
    } else {
      await api.saveProfile(props.path, f);
      note.value = t("edit.saved");
    }
  } catch (e) {
    error.value = asIssue(e);
  } finally {
    busy.value = false;
  }
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
  <form class="panel edit" @submit.prevent="submit">
    <h2>{{ creating ? t("edit.createTitle") : t("edit.title") }}</h2>
    <p class="muted path">{{ path }}</p>
    <IssueList v-if="error" :issues="[error]" />

    <label class="field">
      <span>{{ t("edit.name") }}</span>
      <input v-model="form.title" type="text" required />
    </label>

    <fieldset class="field">
      <legend>{{ t("edit.sources") }}</legend>
      <ul class="roots">
        <li v-for="(r, i) in form.roots" :key="r.path">
          <span class="roots__path">{{ r.path }}</span>
          <input v-model="r.name" type="text" :placeholder="t('edit.sourceAlias')" :aria-label="t('edit.sourceAlias')" />
          <button type="button" class="link" @click="form.roots.splice(i, 1)">{{ t("edit.removeSource") }}</button>
        </li>
      </ul>
      <button type="button" @click="addSource">{{ t("edit.addSource") }}</button>
    </fieldset>

    <label class="field">
      <span>{{ t("edit.excludeDirs") }}</span>
      <input v-model="excludeDirs" type="text" />
    </label>
    <label class="field">
      <span>{{ t("edit.excludeGlobs") }}</span>
      <input v-model="excludeGlobs" type="text" />
    </label>

    <div class="field">
      <span>{{ t("edit.destination") }}</span>
      <div class="row">
        <input v-model="form.destination" type="text" />
        <button type="button" @click="pickDestination">{{ t("edit.pickDestination") }}</button>
      </div>
    </div>

    <details class="field">
      <summary>{{ t("edit.advanced") }}</summary>
      <label class="check"><input v-model="form.blockLongPaths" type="checkbox" /> {{ t("edit.blockLongPaths") }}</label>
      <label class="check"><input v-model="form.verifyAfterCopy" type="checkbox" /> {{ t("edit.verify") }}</label>
      <label class="field">
        <span>{{ t("edit.alwaysSkip") }}</span>
        <input v-model="alwaysSkip" type="text" />
      </label>
      <p class="muted">{{ t("edit.keepForever") }}</p>
      <template v-if="!creating">
        <button type="button" @click="resetDestination">{{ t("edit.resetDestination") }}</button>
        <p class="muted">{{ t("edit.resetDestinationHelp") }}</p>
      </template>
    </details>

    <p v-if="problem" class="warn">{{ problem }}</p>
    <p v-if="note" class="ok">✔ {{ note }}</p>
    <div class="actions">
      <button type="submit" class="primary" :disabled="busy || !!problem">
        {{ creating ? t("edit.create") : t("edit.save") }}
      </button>
      <button type="button" @click="emit('done')">{{ creating ? t("edit.cancel") : t("app.back") }}</button>
    </div>
  </form>
</template>

<style scoped>
.edit {
  display: grid;
  gap: 14px;
}
.edit h2 {
  margin: 0;
}
.path {
  margin: -8px 0 0;
  word-break: break-all;
}
.field {
  display: grid;
  gap: 4px;
  border: none;
  padding: 0;
  margin: 0;
}
.roots {
  list-style: none;
  padding: 0;
  margin: 0 0 8px;
  display: grid;
  gap: 6px;
}
.roots li {
  display: grid;
  grid-template-columns: 1fr 220px auto;
  gap: 8px;
  align-items: center;
}
.roots__path {
  word-break: break-all;
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
  margin: 6px 0;
}
</style>
