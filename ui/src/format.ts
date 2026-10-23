// 台灣慣用格式（ui-workflows.md 原則 5）：2026/10/23 下午 3:05、1.2 GB、12,345。

import { t } from "./i18n/zh-TW";

const dateFmt = new Intl.DateTimeFormat("zh-TW", {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});

export function formatDate(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateFmt.format(d);
}

/// `20261023-1015` 或 `20261023-1015-2` → 可讀的日期時間。
export function formatStamp(stamp: string): string {
  const m = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})/.exec(stamp);
  if (!m) return stamp;
  const [, y, mo, d, h, mi] = m;
  return dateFmt.format(new Date(Number(y), Number(mo) - 1, Number(d), Number(h), Number(mi)));
}

export function formatNumber(n: number): string {
  return n.toLocaleString("zh-TW");
}

export function formatBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return u === 0 ? `${n} B` : `${v.toFixed(1)} ${units[u]}`;
}

export function formatPercent(r: number): string {
  return `${(r * 100).toFixed(1)}%`;
}

export function formatDuration(fromIso: string, toIso: string): string {
  const s = Math.max(0, Math.round((new Date(toIso).getTime() - new Date(fromIso).getTime()) / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}` : `${m}:${String(sec).padStart(2, "0")}`;
}

export function splitList(text: string): string[] {
  return text
    .split(/[,，\n]/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** 預計剩下的時間：「約剩 25 分鐘」「約剩 1 小時 10 分鐘」；還估不出來時「估算剩餘時間中」。 */
export function formatRemaining(seconds: number | null): string {
  if (seconds === null) return t("run.eta.estimating");
  if (seconds < 60) return t("run.eta.underMinute");
  const minutes = Math.ceil(seconds / 60);
  if (minutes < 60) return t("run.eta.minutes", { m: minutes });
  return t("run.eta.hours", { h: Math.floor(minutes / 60), m: minutes % 60 });
}
