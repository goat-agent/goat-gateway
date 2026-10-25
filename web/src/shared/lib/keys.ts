import { defaultKeybindingsHandlerIgnore } from "tinykeys";

const APPLE = /Mac|iPhone|iPad/.test(navigator.userAgent);

export const MOD = APPLE ? "⌘" : "Ctrl";

const OVERLAYS = '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]';

export function typingOrOverlaid(event: KeyboardEvent) {
  if (defaultKeybindingsHandlerIgnore(event)) return true;
  const { target } = event;
  return target instanceof Element && target.closest(OVERLAYS) !== null;
}
