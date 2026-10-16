const APPLE = /Mac|iPhone|iPad/.test(navigator.userAgent);

export const MOD = APPLE ? "⌘" : "Ctrl";
