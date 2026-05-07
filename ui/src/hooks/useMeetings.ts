import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listMeetings } from "@/api/meetings";
import { generateProtocol } from "@/api/protocols";
import { jobSubmit, jobStatus } from "@/api/jobs";
import { queryKeys } from "./queryKeys";
import type { GenerateProtocolArgs, JobSubmitArgs } from "@/api/types";

export function useMeetings() {
  return useQuery({
    queryKey: queryKeys.meetings,
    queryFn: listMeetings,
    staleTime: 30_000,
  });
}

export function useGenerateProtocol() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: GenerateProtocolArgs) => generateProtocol(args),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.meetings }),
  });
}

export function useJobSubmit() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: JobSubmitArgs) => jobSubmit(args),
    onSuccess: () => qc.invalidateQueries({ queryKey: queryKeys.meetings }),
  });
}

export function useJobStatus(id: string | null, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.job(id ?? ""),
    queryFn: () => jobStatus(id!),
    enabled: enabled && id !== null,
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return status === "done" || status === "failed" ? false : 1500;
    },
  });
}
