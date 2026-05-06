export const queryKeys = {
  meetings: ["meetings"] as const,
  meeting: (id: string) => ["meetings", id] as const,
  settings: ["settings"] as const,
  job: (id: string) => ["jobs", id] as const,
  templates: ["templates"] as const,
  diagnostics: ["diagnostics"] as const,
} as const;
