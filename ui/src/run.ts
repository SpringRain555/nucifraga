// 進行中的那一次備份。整個 app 共用一份：執行中畫面、首頁的「進行中」橫幅都讀這裡，
// 所以離開執行中畫面再回來，看到的是同一次備份（ui-workflows「進行中」）。
//
// 事件在 App 啟動時就開始收（initRunEvents），不跟著畫面掛載與卸載。

import { reactive } from "vue";
import {
  api,
  asIssue,
  type Confirmation,
  type Finished,
  type Issue,
  type LockGroup,
  type Progress,
  type RunState,
} from "./api";
import { Eta } from "./eta";
import type { MessageKey } from "./i18n/zh-TW";

/** 執行中畫面的步驟條；首頁的「進行中」也用它說現在在哪一步。 */
export const STEPS: { states: RunState[]; label: MessageKey }[] = [
  { states: ["preflight", "awaitingConfirmation"], label: "run.step.preflight" },
  { states: ["scanning"], label: "run.step.scanning" },
  { states: ["planning"], label: "run.step.planning" },
  { states: ["checkingLocks", "awaitingLockDecision"], label: "run.step.checkingLocks" },
  { states: ["transferring"], label: "run.step.transferring" },
  { states: ["finalizing"], label: "run.step.finalizing" },
];

export function stepOf(state: RunState): MessageKey | null {
  return STEPS.find((s) => s.states.includes(state))?.label ?? null;
}

export const run = reactive({
  /** 這一次的模板；null ＝ 這次開 app 之後還沒備份過。 */
  path: null as string | null,
  title: "",
  state: "idle" as RunState,
  progress: null as Progress | null,
  /** 預計還要幾秒（只在傳輸時有值）。 */
  eta: null as number | null,
  confirmation: null as Confirmation | null,
  locked: null as LockGroup[] | null,
  finished: null as Finished | null,
  startError: null as Issue | null,
  cancelling: false,
});

/** 還在跑（含停下來等使用者回答）。 */
export function isActive(): boolean {
  return run.path !== null && run.finished === null && run.startError === null && run.state !== "idle";
}

/** 停下來等使用者回答。 */
export function isWaiting(): boolean {
  return run.confirmation !== null || run.locked !== null;
}

const eta = new Eta();

export async function initRunEvents() {
  await api.onRunEvent((e) => {
    switch (e.kind) {
      case "state":
        run.state = e.data;
        if (e.data !== "awaitingConfirmation") run.confirmation = null;
        if (e.data !== "awaitingLockDecision") run.locked = null;
        if (e.data !== "transferring") {
          eta.reset();
          run.eta = null;
        }
        break;
      case "progress":
        run.progress = e.data;
        if (run.state === "transferring") {
          run.eta = eta.push(
            { at: Date.now(), files: e.data.filesDone, bytes: e.data.bytesDone },
            e.data.filesTotal,
            e.data.bytesTotal,
          );
        }
        break;
      case "confirmation":
        run.confirmation = e.data;
        break;
      case "locked":
        run.locked = e.data;
        break;
      case "issue":
        break; // 結果畫面一次列出全部，不在進行中逐則跳出
    }
  });
  await api.onRunFinished((f) => {
    run.finished = f;
    run.eta = null;
  });
}

export async function startRun(path: string, title: string) {
  Object.assign(run, {
    path,
    title,
    state: "idle",
    progress: null,
    eta: null,
    confirmation: null,
    locked: null,
    finished: null,
    startError: null,
    cancelling: false,
  });
  eta.reset();
  try {
    await api.startRun(path);
  } catch (e) {
    run.startError = asIssue(e);
  }
}

export async function cancelRun() {
  run.cancelling = true;
  await api.cancelRun(false);
}
