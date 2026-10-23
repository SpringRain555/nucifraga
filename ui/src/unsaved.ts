// 模板編輯畫面的「未儲存的修改」：編輯畫面登記，App 的換頁與關視窗都先問這裡（ui-workflows「模板編輯」）。

import { reactive, watch } from "vue";
import { api } from "./api";

export const unsaved = reactive({
  dirty: false,
  /** 編輯畫面提供的「儲存」；成功回 true。 */
  save: null as null | (() => Promise<boolean>),
});

// 後端也要知道，按視窗的關閉鈕時才攔得下來。
watch(
  () => unsaved.dirty,
  (d) => void api.setUnsaved(d),
);
