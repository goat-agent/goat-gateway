import { useCallback, useState, type ReactNode } from "react";
import { useShortcuts } from "../model/use-shortcuts";
import { CommandMenu } from "./command-menu";
import { ShortcutSheet } from "./shortcut-sheet";
import { Sidebar } from "./sidebar";

export function AppShell({ children }: { children: ReactNode }) {
  const [bumps, setBumps] = useState(0);
  useShortcuts({ onRepeat: useCallback(() => setBumps((count) => count + 1), []) });

  return (
    <div className="flex h-full bg-base">
      <Sidebar />
      <main className="my-2 mr-2 flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg hairline border-line bg-panel shadow-low">
        {children}
      </main>
      <CommandMenu bumps={bumps} />
      <ShortcutSheet />
    </div>
  );
}
