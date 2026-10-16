import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

const merged = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [{ text: ["micro", "mini", "small", "regular", "large", "headline"] }],
    },
  },
});

export function cn(...classes: ClassValue[]) {
  return merged(clsx(classes));
}
