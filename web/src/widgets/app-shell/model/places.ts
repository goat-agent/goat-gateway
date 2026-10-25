import {
  Boxes,
  ChartColumn,
  House,
  KeyRound,
  PlugZap,
  Rows3,
  Settings,
  Users,
  type LucideIcon,
} from "lucide-react";
import { useMe } from "@/entities/user";

export const SECTIONS = ["Traffic", "Gateway"] as const;

export type Place = {
  to: string;
  label: string;
  icon: LucideIcon;
  end: boolean;
  letter: string;
  section: (typeof SECTIONS)[number] | null;
  admin: boolean;
};

export const PLACES = [
  { to: "/", label: "Home", icon: House, end: true, letter: "H", section: "Traffic", admin: false },
  { to: "/requests", label: "Requests", icon: Rows3, end: false, letter: "R", section: "Traffic", admin: false },
  { to: "/usage", label: "Usage", icon: ChartColumn, end: false, letter: "U", section: "Traffic", admin: false },
  { to: "/accounts", label: "Accounts", icon: PlugZap, end: false, letter: "A", section: "Gateway", admin: false },
  { to: "/keys", label: "Keys", icon: KeyRound, end: false, letter: "K", section: "Gateway", admin: false },
  { to: "/providers", label: "Providers", icon: Boxes, end: false, letter: "V", section: "Gateway", admin: false },
  { to: "/people", label: "People", icon: Users, end: false, letter: "P", section: "Gateway", admin: true },
  { to: "/settings", label: "Settings", icon: Settings, end: false, letter: "S", section: null, admin: false },
] as const satisfies readonly Place[];

export function usePlaces(): readonly Place[] {
  const admin = useMe((store) => store.me?.admin ?? false);
  return PLACES.filter((place) => admin || !place.admin);
}
