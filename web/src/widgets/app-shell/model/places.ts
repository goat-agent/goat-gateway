import {
  Boxes,
  ChartColumn,
  KeyRound,
  LayoutGrid,
  PlugZap,
  Rows3,
  Settings,
  Users,
  type LucideIcon,
} from "lucide-react";
import { useMe } from "@/entities/user";

export const SECTIONS = ["View", "Configure"] as const;

export type Place = {
  to: string;
  label: string;
  icon: LucideIcon;
  end: boolean;
  letter: string;
  section: (typeof SECTIONS)[number];
  admin: boolean;
};

export const PLACES = [
  { to: "/", label: "Overview", icon: LayoutGrid, end: true, letter: "O", section: "View", admin: false },
  { to: "/requests", label: "Requests", icon: Rows3, end: false, letter: "R", section: "View", admin: false },
  { to: "/usage", label: "Usage", icon: ChartColumn, end: false, letter: "U", section: "View", admin: false },
  { to: "/accounts", label: "Accounts", icon: PlugZap, end: false, letter: "A", section: "Configure", admin: false },
  { to: "/keys", label: "Keys", icon: KeyRound, end: false, letter: "K", section: "Configure", admin: false },
  { to: "/providers", label: "Providers", icon: Boxes, end: false, letter: "V", section: "Configure", admin: false },
  { to: "/people", label: "People", icon: Users, end: false, letter: "P", section: "Configure", admin: true },
  { to: "/settings", label: "Settings", icon: Settings, end: false, letter: "S", section: "Configure", admin: false },
] as const satisfies readonly Place[];

export function usePlaces(): readonly Place[] {
  const admin = useMe((store) => store.me?.admin ?? false);
  return PLACES.filter((place) => admin || !place.admin);
}
