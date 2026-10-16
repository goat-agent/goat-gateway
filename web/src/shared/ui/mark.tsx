import { cn } from "@/shared/lib/cn";

export function Mark({ className }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={cn(
        "grid size-5 shrink-0 place-items-center rounded-[5px] bg-accent text-micro leading-none font-semibold text-accent-text shadow-low select-none",
        className,
      )}
    >
      G
    </span>
  );
}
