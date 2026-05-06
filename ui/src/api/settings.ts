import { invokeCmd } from "./client";
import type { SettingsDto } from "./types";

/** mirrors: settings_get() → SettingsDto (anthropic_api_key always null in response) */
export function settingsGet(): Promise<SettingsDto> {
  return invokeCmd("settings_get");
}

/**
 * mirrors: settings_set(dto) → ()
 * Pass anthropic_api_key: null to leave the key unchanged.
 * Pass anthropic_api_key: "" to clear it.
 */
export function settingsSet(dto: SettingsDto): Promise<void> {
  return invokeCmd("settings_set", { dto });
}
