import type { LucideIcon } from "lucide-react";
import { useEffect, useRef } from "react";
import { create } from "zustand";

export type Command = {
  id: string;
  label: string;
  group: string;
  icon?: LucideIcon;
  keys?: readonly string[];
  then?: boolean;
  perform: () => void;
};

type Commands = {
  registered: Command[];
  open: boolean;
  setOpen: (open: boolean) => void;
  show: () => void;
  toggle: () => void;
  register: (command: Command) => () => void;
};

export const useCommands = create<Commands>((set) => ({
  registered: [],
  open: false,
  setOpen: (open) => set({ open }),
  show: () => set({ open: true }),
  toggle: () => set((held) => ({ open: !held.open })),
  register: (command) => {
    set((held) => ({
      registered: [...held.registered.filter((entry) => entry.id !== command.id), command],
    }));
    return () =>
      set((held) => ({ registered: held.registered.filter((entry) => entry !== command) }));
  },
}));

export function useCommand(command: Command, enabled = true) {
  const perform = useRef(command.perform);
  perform.current = command.perform;

  const { id, label, group, icon, keys, then } = command;
  const pressed = keys?.join(" ");

  useEffect(() => {
    if (!enabled) return;
    return useCommands.getState().register({
      id,
      label,
      group,
      icon,
      keys: pressed?.split(" "),
      then,
      perform: () => perform.current(),
    });
  }, [enabled, id, label, group, icon, pressed, then]);
}
