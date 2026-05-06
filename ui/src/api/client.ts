import { invoke } from "@tauri-apps/api/core";

/**
 * Thin wrapper over Tauri invoke that normalises errors.
 * Tauri throws strings on the Rust side — we convert them to Error objects.
 */
export async function invokeCmd<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    if (typeof err === "string") throw new Error(err);
    throw err;
  }
}
