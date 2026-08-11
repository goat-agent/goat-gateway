const SLOTS = 8;
const assigned = new Map<string, number>();

export function seriesColor(key: string) {
  return `var(--series-${slotFor(key)})`;
}

function slotFor(key: string) {
  const held = assigned.get(key);
  if (held !== undefined) return held;

  const taken = new Set(assigned.values());
  let slot = (hash(key) % SLOTS) + 1;
  for (let step = 0; step < SLOTS && taken.has(slot); step += 1) {
    slot = (slot % SLOTS) + 1;
  }
  assigned.set(key, slot);
  return slot;
}

function hash(key: string) {
  let value = 0;
  for (let index = 0; index < key.length; index += 1) {
    value = (value * 31 + key.charCodeAt(index)) >>> 0;
  }
  return value;
}
