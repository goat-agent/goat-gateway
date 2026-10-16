import { Autocomplete } from "@base-ui/react/autocomplete";
import { Dialog } from "@base-ui/react/dialog";
import { Keyboard, LogOut, Moon, Search, Sun, type LucideIcon } from "lucide-react";
import { useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { useCommands, type Command } from "@/shared/model/commands";
import { Keys } from "@/shared/ui";
import { useTheme } from "@/features/toggle-theme";
import { signOut } from "@/features/sign-out";
import { PLACES } from "../model/places";
import { useSheet } from "../model/sheet";

type Entry = {
  value: string;
  label: string;
  icon?: LucideIcon;
  keys?: readonly string[];
  then?: boolean;
  perform: () => void;
};

type Section = { value: string; items: Entry[] };

export function CommandMenu({ bumps }: { bumps: number }) {
  const open = useCommands((store) => store.open);
  const setOpen = useCommands((store) => store.setOpen);
  const registered = useCommands((store) => store.registered);
  const theme = useTheme((store) => store.theme);
  const flip = useTheme((store) => store.flip);
  const navigate = useNavigate();
  const popup = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (bumps === 0) return;
    popup.current?.animate([{ scale: 1 }, { scale: 0.985 }, { scale: 1 }], {
      duration: 150,
      easing: "cubic-bezier(0.25, 0.46, 0.45, 0.94)",
    });
  }, [bumps]);

  const sections: Section[] = [
    {
      value: "Go to",
      items: PLACES.map((place) => ({
        value: `go ${place.to}`,
        label: `Go to ${place.label}`,
        icon: place.icon,
        keys: ["G", place.letter],
        then: true,
        perform: () => navigate(place.to),
      })),
    },
    ...grouped(registered),
    {
      value: "General",
      items: [
        {
          value: "theme",
          label: theme === "dark" ? "Switch to the light theme" : "Switch to the dark theme",
          icon: theme === "dark" ? Sun : Moon,
          perform: flip,
        },
        {
          value: "shortcuts",
          label: "Show keyboard shortcuts",
          icon: Keyboard,
          keys: ["?"],
          perform: () => useSheet.getState().setOpen(true),
        },
        { value: "sign out", label: "Sign out", icon: LogOut, perform: signOut },
      ],
    },
  ];

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Portal>
        <Dialog.Backdrop className="scrim-motion fixed inset-0 z-[var(--layer-command)] bg-scrim" />
        <Dialog.Viewport className="fixed inset-0 z-[var(--layer-command)] flex items-start justify-center px-4 pt-[13vh]">
          <Dialog.Popup
            ref={popup}
            aria-label="Command menu"
            className="dialog-motion flex max-h-[min(30rem,72vh)] w-[min(40rem,100%)] origin-top flex-col overflow-hidden rounded-xl hairline border-line-menu bg-overlay text-ink shadow-high outline-none select-none"
          >
            <Autocomplete.Root open inline items={sections} autoHighlight="always" keepHighlight>
              <div className="flex h-[var(--command-input)] shrink-0 items-center gap-3 hairline-b border-line px-4">
                <Search className="size-4 shrink-0 text-ink-muted" strokeWidth={1.75} />
                <Autocomplete.Input
                  aria-label="Search commands"
                  placeholder="Type a command or search…"
                  className="h-full w-full min-w-0 bg-transparent text-regular text-ink-title outline-none placeholder:text-ink-muted"
                />
              </div>

              <Autocomplete.Empty className="px-4 py-10 text-center text-small text-ink-secondary empty:hidden">
                No command matches that.
              </Autocomplete.Empty>

              <Autocomplete.List className="min-h-0 overflow-y-auto p-1.5 empty:hidden">
                {(section: Section) => (
                  <Autocomplete.Group key={section.value} items={section.items} className="pb-1">
                    <Autocomplete.GroupLabel className="flex h-8 items-center px-2.5 text-mini font-medium text-ink-secondary">
                      {section.value}
                    </Autocomplete.GroupLabel>
                    <Autocomplete.Collection>
                      {(entry: Entry) => (
                        <Autocomplete.Item
                          key={entry.value}
                          value={entry}
                          onClick={() => {
                            setOpen(false);
                            entry.perform();
                          }}
                          className="hover-fade flex h-10 cursor-default items-center gap-3 rounded-lg px-2.5 text-small text-ink outline-none data-[highlighted]:bg-menu-hover data-[highlighted]:text-ink-title"
                        >
                          {entry.icon ? (
                            <entry.icon className="size-4 shrink-0 text-ink-secondary" strokeWidth={1.75} />
                          ) : (
                            <span className="size-4 shrink-0" />
                          )}
                          <span className="min-w-0 grow truncate">{entry.label}</span>
                          {entry.keys ? <Keys keys={entry.keys} then={entry.then} /> : null}
                        </Autocomplete.Item>
                      )}
                    </Autocomplete.Collection>
                  </Autocomplete.Group>
                )}
              </Autocomplete.List>

              <div className="flex h-9 shrink-0 items-center gap-4 hairline-t border-line px-4 text-micro text-ink-secondary">
                <span className="flex items-center gap-1.5">
                  <Keys keys={["↑", "↓"]} /> to move
                </span>
                <span className="flex items-center gap-1.5">
                  <Keys keys={["↵"]} /> to run
                </span>
                <span className="flex items-center gap-1.5">
                  <Keys keys={["esc"]} /> to close
                </span>
              </div>
            </Autocomplete.Root>
            <Dialog.Close className="sr-only">Close the command menu</Dialog.Close>
          </Dialog.Popup>
        </Dialog.Viewport>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function grouped(commands: Command[]): Section[] {
  const sections: Section[] = [];
  for (const command of commands) {
    const entry: Entry = {
      value: command.id,
      label: command.label,
      icon: command.icon,
      keys: command.keys,
      then: command.then,
      perform: command.perform,
    };
    const held = sections.find((section) => section.value === command.group);
    if (held) held.items.push(entry);
    else sections.push({ value: command.group, items: [entry] });
  }
  return sections;
}
