import { invokeCmd } from "./client";

/** mirrors: transcribe_file(path) → String (raw transcript text) */
export function transcribeFile(path: string): Promise<string> {
  return invokeCmd("transcribe_file", { path });
}
