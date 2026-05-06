import { invokeCmd } from "./client";
import type { DiagnosticsDto } from "./types";

/** mirrors: diagnostics() → DiagnosticsDto */
export function diagnostics(): Promise<DiagnosticsDto> {
  return invokeCmd("diagnostics");
}

/** mirrors: diagnostics_logs() → Vec<String> */
export function diagnosticsLogs(): Promise<string[]> {
  return invokeCmd("diagnostics_logs");
}

/** mirrors: open_path(path) → () */
export function openPath(path: string): Promise<void> {
  return invokeCmd("open_path", { path });
}

/** mirrors: show_debug_window() → () */
export function showDebugWindow(): Promise<void> {
  return invokeCmd("show_debug_window");
}
