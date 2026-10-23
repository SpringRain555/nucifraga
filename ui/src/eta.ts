// 預計完成時間（ui-workflows「執行中」）。
//
// 只看位元組會被小檔騙（時間花在每個檔案的固定開銷），只看檔案數會被大檔騙，
// 所以兩個各估一次，取平均。速度用「最近一分鐘」與「從頭到現在」各半：
// 只看最近的，卡在一個大檔案時速度趨近 0、估計會暴衝；只看整體的，換到慢的資料夾時跟不上。
// 最後再做指數平滑，數字才不會一直跳。

export interface Sample {
  at: number;
  files: number;
  bytes: number;
}

/** 最近多久的速度。 */
const WINDOW_MS = 60_000;
/** 傳了多久才開始顯示（太早的速度不準）。 */
const WARMUP_MS = 10_000;
/** 平滑：新估計佔的比例。 */
const ALPHA = 0.2;

export class Eta {
  private samples: Sample[] = [];
  private start: Sample | null = null;
  private smoothed: number | null = null;

  reset() {
    this.samples = [];
    this.start = null;
    this.smoothed = null;
  }

  /** 加一筆進度，回傳剩下的秒數；還估不出來時回 null。 */
  push(s: Sample, filesTotal: number, bytesTotal: number): number | null {
    this.start ??= s;
    this.samples.push(s);
    while (this.samples.length > 2 && s.at - (this.samples[0]?.at ?? s.at) > WINDOW_MS) this.samples.shift();
    if (s.at - this.start.at < WARMUP_MS) return null;

    const recent = this.samples[0] ?? s;
    const rate = (pick: (x: Sample) => number) => {
      const r = (from: Sample) => (s.at > from.at ? (pick(s) - pick(from)) / ((s.at - from.at) / 1000) : 0);
      return (r(recent) + r(this.start ?? s)) / 2;
    };
    const estimates: number[] = [];
    const fileRate = rate((x) => x.files);
    const byteRate = rate((x) => x.bytes);
    if (fileRate > 0) estimates.push((filesTotal - s.files) / fileRate);
    if (byteRate > 0) estimates.push((bytesTotal - s.bytes) / byteRate);
    if (estimates.length === 0) return this.smoothed;

    const raw = estimates.reduce((a, b) => a + b, 0) / estimates.length;
    this.smoothed = this.smoothed === null ? raw : this.smoothed * (1 - ALPHA) + raw * ALPHA;
    return Math.max(0, this.smoothed);
  }
}
