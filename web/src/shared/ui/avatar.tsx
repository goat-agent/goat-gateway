import { cn } from "@/shared/lib/cn";

export function Avatar({ name, className }: { name: string; className?: string }) {
  const initials = name
    .split(/\s+/)
    .filter((word) => word !== "")
    .slice(0, 2)
    .map((word) => Array.from(word)[0]?.toUpperCase() ?? "")
    .join("");

  return (
    <span
      aria-hidden
      className={cn(
        "grid size-5 shrink-0 place-items-center rounded-full hairline border-line-solid bg-raised-hover text-[10px] leading-none font-semibold text-ink-secondary select-none",
        className,
      )}
    >
      {initials || "?"}
    </span>
  );
}
