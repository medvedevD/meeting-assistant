import { cn } from "@/lib/utils";

interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  variant?: "default" | "success" | "muted" | "destructive";
}

export function Badge({ className, variant = "default", ...props }: BadgeProps) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium",
        variant === "default" && "bg-[var(--primary)] text-[var(--primary-foreground)]",
        variant === "success" && "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
        variant === "muted" && "bg-[var(--muted)] text-[var(--muted-foreground)]",
        variant === "destructive" && "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
        className
      )}
      {...props}
    />
  );
}
