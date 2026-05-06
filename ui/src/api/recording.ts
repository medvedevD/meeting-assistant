import { invokeCmd } from "./client";
import type { RecordingDto, StartRecordingArgs } from "./types";

/** mirrors: recording_start(name?, source?, echo_cancel?) → RecordingDto */
export function startRecording(args: StartRecordingArgs = {}): Promise<RecordingDto> {
  return invokeCmd("recording_start", {
    name: args.name,
    source: args.source,
    echo_cancel: args.echo_cancel,
  });
}

/** mirrors: recording_stop(id) → RecordingDto */
export function stopRecording(id: string): Promise<RecordingDto> {
  return invokeCmd("recording_stop", { id });
}
