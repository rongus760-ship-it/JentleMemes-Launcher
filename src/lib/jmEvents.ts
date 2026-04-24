/** Тосты и тема — те же события, что раньше в React. */
export function showToast(msg: string) {
  window.dispatchEvent(new CustomEvent("jm_toast", { detail: msg }));
}
