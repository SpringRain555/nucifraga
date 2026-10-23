// 後端命令的型別與包裝。形狀與 src-tauri/src/commands.rs、crates/core 的 Serialize 一對一。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Level = "fatal" | "error" | "warning" | "info";

export interface Issue {
  code: string;
  level: Level;
  message: string;
  action: string;
  root?: string;
  path?: string;
  detail?: string;
}

export type Outcome = "completed" | "completedWithWarnings" | "failed" | "cancelled";

export interface Counts {
  scanned: number;
  unchanged: number;
  copied: number;
  updated: number;
  versioned: number;
  verified: number;
  skipped: number;
  errors: number;
}

export interface Report {
  runId: string;
  profile: string;
  destination?: string;
  outcome: Outcome;
  startedAt: string;
  finishedAt: string;
  counts: Counts;
  bytesCopied: number;
  issues: Issue[];
}

export interface ProfileCard {
  path: string;
  title: string;
  destination?: string;
  roots: string[];
  destinationConnected: boolean;
  /** 模板資料夾外的模板（只記路徑）。 */
  external: boolean;
  error?: Issue;
  last?: Report;
}

export interface Home {
  /** null ＝ 第一次使用，還沒選模板資料夾。 */
  profilesFolder?: string | null;
  suggestedFolder: string;
  folderIssue?: Issue | null;
  profiles: ProfileCard[];
  settingsWasBroken: boolean;
  running: boolean;
}

export interface RootForm {
  path: string;
  name?: string | null;
}

export interface ProfileForm {
  title: string;
  destination: string;
  roots: RootForm[];
  excludeDirNames: string[];
  excludeGlobs: string[];
  blockLongPaths: boolean;
  verifyAfterCopy: boolean;
  alwaysSkipLocked: string[];
}

export type RunState =
  | "idle"
  | "preflight"
  | "scanning"
  | "planning"
  | "awaitingConfirmation"
  | "checkingLocks"
  | "awaitingLockDecision"
  | "transferring"
  | "finalizing"
  | "completed"
  | "completedWithWarnings"
  | "failed"
  | "cancelled";

export interface Progress {
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
  current?: string | null;
}

export interface MassVersioning {
  root: string;
  count: number;
  ratio: number;
  sample: string[];
}

export type Confirmation =
  | { kind: "massVersioning"; items: MassVersioning[] }
  | { kind: "adoptDestination"; destination: string; destinationId: string };

export interface LockGroup {
  app: string;
  files: { root: string; rel: string }[];
}

export type RunEvent =
  | { kind: "state"; data: RunState }
  | { kind: "progress"; data: Progress }
  | { kind: "issue"; data: Issue }
  | { kind: "confirmation"; data: Confirmation }
  | { kind: "locked"; data: LockGroup[] };

export interface Finished {
  report: Report;
  reportDir?: string;
}

export interface VersionStamp {
  name: string;
  files: number;
  bytes: number;
}

export type LockAnswer = "retry" | "skip" | "skipAndRemember" | "cancel";

export const api = {
  info: () => invoke<{ version: string; identifier: string }>("app_info"),
  ready: () => invoke<void>("app_ready"),
  home: () => invoke<Home>("home"),
  addProfile: (path: string) => invoke<void>("add_profile", { path }),
  createProfile: (title: string, destination: string, roots: string[]) =>
    invoke<string>("create_profile", { title, destination, roots }),
  setProfilesFolder: (path: string, moveExisting: boolean) =>
    invoke<Issue[]>("set_profiles_folder", { path, moveExisting }),
  openProfilesFolder: () => invoke<void>("open_profiles_folder"),
  forgetProfile: (path: string) => invoke<void>("forget_profile", { path }),
  getProfile: (path: string) => invoke<ProfileForm>("get_profile", { path }),
  saveProfile: (path: string, form: ProfileForm) => invoke<void>("save_profile", { path, form }),
  resetDestination: (path: string) => invoke<void>("reset_destination", { path }),
  startRun: (path: string) => invoke<void>("start_run", { path }),
  answerConfirmation: (yes: boolean) => invoke<void>("answer_confirmation", { yes }),
  answerLocks: (answer: LockAnswer) => invoke<void>("answer_locks", { answer }),
  cancelRun: (thenQuit = false) => invoke<void>("cancel_run", { thenQuit }),
  history: (path: string) => invoke<Report[]>("history", { path }),
  versions: (path: string) => invoke<VersionStamp[]>("versions", { path }),
  deleteVersion: (path: string, stamp: string) => invoke<VersionStamp>("delete_version", { path, stamp }),
  openFolder: (path: string, place: "current" | "report" | "version", stamp?: string) =>
    invoke<void>("open_folder", { path, place, stamp }),
  onRunEvent: (f: (e: RunEvent) => void): Promise<UnlistenFn> => listen<RunEvent>("run-event", (e) => f(e.payload)),
  onRunFinished: (f: (e: Finished) => void): Promise<UnlistenFn> => listen<Finished>("run-finished", (e) => f(e.payload)),
  onCloseRequested: (f: () => void): Promise<UnlistenFn> => listen("close-requested", () => f()),
};

/// 後端回的錯誤一定是 Issue；萬一不是（例如 IPC 本身壞掉），包成一個 ENG-002 的樣子顯示。
export function asIssue(e: unknown): Issue {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return e as Issue;
  }
  return { code: "ENG-002", level: "fatal", message: String(e), action: "" };
}
