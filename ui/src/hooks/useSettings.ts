import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { settingsGet, settingsSet } from "@/api/settings";
import { queryKeys } from "./queryKeys";
import type { SettingsDto } from "@/api/types";

export function useSettings() {
  return useQuery({
    queryKey: queryKeys.settings,
    queryFn: settingsGet,
  });
}

export function useSettingsMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (dto: SettingsDto) => settingsSet(dto),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.settings }),
  });
}
