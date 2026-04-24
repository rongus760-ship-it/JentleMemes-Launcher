/** KeyboardEvent.code values that are only modifiers — no shortcut token. */
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

function browserCodeToHotkeyKey(code: string): string | null {
  if (code.startsWith("Key") && code.length === 4) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);

  const map: Record<string, string> = {
    Backquote: "Backquote",
    Minus: "Minus",
    Equal: "Equal",
    BracketLeft: "BracketLeft",
    BracketRight: "BracketRight",
    Backslash: "Backslash",
    Semicolon: "Semicolon",
    Quote: "Quote",
    Comma: "Comma",
    Period: "Period",
    Slash: "Slash",
    IntlBackslash: "IntlBackslash",
    Backspace: "Backspace",
    Tab: "Tab",
    Enter: "Enter",
    Space: "Space",
    Escape: "Escape",
    ArrowUp: "ArrowUp",
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
    Insert: "Insert",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    NumpadDecimal: "NumDecimal",
    NumpadDivide: "NumDivide",
    NumpadMultiply: "NumMultiply",
    NumpadSubtract: "NumSubtract",
    NumpadAdd: "NumAdd",
    NumpadEnter: "NumEnter",
    NumpadEqual: "NumEqual",
  };
  if (map[code]) return map[code];

  const numpadDigit: Record<string, string> = {
    Numpad0: "Num0",
    Numpad1: "Num1",
    Numpad2: "Num2",
    Numpad3: "Num3",
    Numpad4: "Num4",
    Numpad5: "Num5",
    Numpad6: "Num6",
    Numpad7: "Num7",
    Numpad8: "Num8",
    Numpad9: "Num9",
  };
  if (numpadDigit[code]) return numpadDigit[code];

  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  return null;
}

/**
 * Builds a string for @tauri-apps/plugin-global-shortcut from a keydown event.
 * Returns null if the event is not a bindable key (e.g. modifier-only).
 */
export function keyboardEventToTauriShortcut(e: KeyboardEvent): string | null {
  if (e.repeat) return null;
  if (MODIFIER_CODES.has(e.code)) return null;

  const keyTok = browserCodeToHotkeyKey(e.code);
  if (!keyTok) return null;

  const parts: string[] = [];
  if (e.shiftKey) parts.push("Shift");
  if (e.ctrlKey) parts.push("Control");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Super");

  parts.push(keyTok);
  return parts.join("+");
}
