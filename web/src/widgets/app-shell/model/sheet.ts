import { create } from "zustand";

type Sheet = {
  open: boolean;
  setOpen: (open: boolean) => void;
};

export const useSheet = create<Sheet>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
}));
