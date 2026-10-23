// UI 字串的唯一位置（REQ-0008）。用詞以 docs/architecture/glossary.md 為準；
// 錯誤訊息不在這裡 —— 它們來自 docs/architecture/error-codes.md，由 Rust 端渲染好再送過來。
//
// key 的形狀是「畫面.元件.狀態」。

export const zhTW = {
  "app.title": "Nucifraga",
  "app.version": "版本 {version}",
  "home.empty.title": "還沒有任何模板",
  "home.empty.body":
    "模板記錄「要備份哪些資料夾、排除什麼、備份到哪顆碟」。加入一份之後，每次備份只要按一下。",
  "home.empty.addExisting": "加入現有的模板檔",
  "home.empty.createNew": "建立新模板",
  "home.notReady": "這個功能還在開發中（M1）。",
} as const;

export type MessageKey = keyof typeof zhTW;

export function t(key: MessageKey, args: Record<string, string> = {}): string {
  let text: string = zhTW[key];
  for (const [name, value] of Object.entries(args)) {
    text = text.replaceAll(`{${name}}`, value);
  }
  return text;
}
