import {
  ChartColumn,
  KeyRound,
  LayoutGrid,
  PlugZap,
  Rows3,
  Settings,
  type LucideIcon,
} from "lucide-react";

export const SECTIONS = ["View", "Configure"] as const;

export type Place = {
  to: string;
  label: string;
  icon: LucideIcon;
  end: boolean;
  letter: string;
  section: (typeof SECTIONS)[number];
};

export const PLACES = [
  { to: "/", label: "Overview", icon: LayoutGrid, end: true, letter: "O", section: "View" },
  { to: "/requests", label: "Requests", icon: Rows3, end: false, letter: "R", section: "View" },
  { to: "/usage", label: "Usage", icon: ChartColumn, end: false, letter: "U", section: "View" },
  { to: "/accounts", label: "Accounts", icon: PlugZap, end: false, letter: "A", section: "Configure" },
  { to: "/keys", label: "Keys", icon: KeyRound, end: false, letter: "K", section: "Configure" },
  { to: "/settings", label: "Settings", icon: Settings, end: false, letter: "S", section: "Configure" },
] as const satisfies readonly Place[];
