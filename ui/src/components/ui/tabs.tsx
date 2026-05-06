import React, { createContext, useContext, useState } from "react";
import { cn } from "@/lib/utils";

interface TabsContextValue {
  active: string;
  setActive: (v: string) => void;
}

const TabsCtx = createContext<TabsContextValue>({ active: "", setActive: () => {} });

export function Tabs({
  defaultValue,
  value,
  onValueChange,
  children,
  className,
}: {
  defaultValue?: string;
  value?: string;
  onValueChange?: (v: string) => void;
  children: React.ReactNode;
  className?: string;
}) {
  const [internal, setInternal] = useState(defaultValue ?? "");
  const active = value ?? internal;
  const setActive = (v: string) => {
    setInternal(v);
    onValueChange?.(v);
  };
  return (
    <TabsCtx.Provider value={{ active, setActive }}>
      <div className={cn("flex flex-col", className)}>{children}</div>
    </TabsCtx.Provider>
  );
}

export function TabsList({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        "flex items-center gap-0.5 border-b border-[var(--border)] px-1",
        className
      )}
    >
      {children}
    </div>
  );
}

export function TabsTrigger({
  value,
  children,
  className,
}: {
  value: string;
  children: React.ReactNode;
  className?: string;
}) {
  const { active, setActive } = useContext(TabsCtx);
  return (
    <button
      onClick={() => setActive(value)}
      className={cn(
        "px-3 py-2 text-sm font-medium transition-colors border-b-2 -mb-px",
        active === value
          ? "border-[var(--primary)] text-[var(--foreground)]"
          : "border-transparent text-[var(--muted-foreground)] hover:text-[var(--foreground)]",
        className
      )}
    >
      {children}
    </button>
  );
}

export function TabsContent({
  value,
  children,
  className,
}: {
  value: string;
  children: React.ReactNode;
  className?: string;
}) {
  const { active } = useContext(TabsCtx);
  if (active !== value) return null;
  return <div className={cn("flex-1 overflow-y-auto", className)}>{children}</div>;
}
