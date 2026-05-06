import { useState, useCallback } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { startRecording, stopRecording } from "@/api/recording";
import { queryKeys } from "./queryKeys";
import type { RecordingDto, StartRecordingArgs } from "@/api/types";

export type RecordingState =
  | { status: "idle" }
  | { status: "recording"; dto: RecordingDto; startedAt: number }
  | { status: "stopped"; dto: RecordingDto };

export function useRecording() {
  const qc = useQueryClient();
  const [state, setState] = useState<RecordingState>({ status: "idle" });

  const startMutation = useMutation({
    mutationFn: (args: StartRecordingArgs) => startRecording(args),
    onSuccess: (dto) => {
      setState({ status: "recording", dto, startedAt: Date.now() });
    },
  });

  const stopMutation = useMutation({
    mutationFn: (id: string) => stopRecording(id),
    onSuccess: (dto) => {
      setState({ status: "stopped", dto });
      qc.invalidateQueries({ queryKey: queryKeys.meetings });
    },
  });

  const start = useCallback(
    (args: StartRecordingArgs) => startMutation.mutateAsync(args),
    [startMutation]
  );

  const stop = useCallback(() => {
    if (state.status !== "recording") return;
    return stopMutation.mutateAsync(state.dto.id);
  }, [state, stopMutation]);

  const reset = useCallback(() => setState({ status: "idle" }), []);

  return {
    state,
    start,
    stop,
    reset,
    isStarting: startMutation.isPending,
    isStopping: stopMutation.isPending,
    startError: startMutation.error,
    stopError: stopMutation.error,
  };
}
