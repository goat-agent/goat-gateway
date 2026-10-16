import { Dialog, Keys } from "@/shared/ui";
import { MOD } from "@/shared/lib/keys";
import { PLACES } from "../model/places";
import { useSheet } from "../model/sheet";

type Shortcut = { label: string; keys: readonly string[]; then?: boolean };

const GENERAL: readonly Shortcut[] = [
  { label: "Open the command menu", keys: [MOD, "K"] },
  { label: "Show keyboard shortcuts", keys: ["?"] },
  { label: "Close whatever is open", keys: ["Esc"] },
];

const GOING: readonly Shortcut[] = PLACES.map((place) => ({
  label: `Go to ${place.label}`,
  keys: ["G", place.letter],
  then: true,
}));

export function ShortcutSheet() {
  const open = useSheet((store) => store.open);
  const setOpen = useSheet((store) => store.setOpen);

  return (
    <Dialog
      open={open}
      onClose={() => setOpen(false)}
      title="Keyboard shortcuts"
      description="None of these fire while you are typing in a field."
      width="narrow"
    >
      <div className="flex flex-col gap-5">
        <Listing title="General" shortcuts={GENERAL} />
        <Listing title="Navigation" shortcuts={GOING} />
      </div>
    </Dialog>
  );
}

function Listing({ title, shortcuts }: { title: string; shortcuts: readonly Shortcut[] }) {
  return (
    <section className="flex flex-col">
      <h3 className="m-0 pb-1 text-mini font-medium text-ink-secondary">{title}</h3>
      {shortcuts.map((shortcut) => (
        <div
          key={shortcut.label}
          className="flex h-9 items-center justify-between gap-4 hairline-b border-line-subtle text-small text-ink last:border-b-0"
        >
          {shortcut.label}
          <Keys keys={shortcut.keys} then={shortcut.then} />
        </div>
      ))}
    </section>
  );
}
