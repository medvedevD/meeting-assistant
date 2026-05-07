/**
 * TypeScript mirrors of Rust DTOs from ui/src-tauri/src/lib.rs.
 * Field names follow serde snake_case serialisation.
 */

// MeetingDto (lib.rs:80)
export interface MeetingDto {
  id: string;
  name: string;
  audio_path: string;
  has_transcript: boolean;
  has_protocol: boolean;
  created_at: number; // unix timestamp (i64)
}

// RecordingDto (lib.rs:89)
export interface RecordingDto {
  id: string;
  name: string;
  audio_path: string;
}

// ProtocolDto (lib.rs:96)
export interface ProtocolDto {
  markdown: string;
}

// JobDto (lib.rs:101)
export interface JobDto {
  id: string;
  meeting_id: string;
  status: "pending" | "running" | "done" | "failed";
  attempts: number;
  last_error: string | null;
}

// SettingsPaths (lib.rs:110)
export interface SettingsPaths {
  model: string | null;
  db: string | null;
  recordings: string | null;
  prompts: string | null;
}

// RecordingPrefs (lib.rs:117)
export interface RecordingPrefs {
  source: "mic" | "system" | "mixed";
  echo_cancel: boolean;
}

// SettingsDto (lib.rs:124)
export interface SettingsDto {
  paths: SettingsPaths;
  /** null when fetched (never exposed); non-null to update (empty string = clear) */
  anthropic_api_key: string | null;
  recording: RecordingPrefs;
  default_template: string | null;
}

// PathInfoDto (lib.rs:131)
export interface PathInfoDto {
  path: string;
  exists: boolean;
  writable: boolean;
  size_bytes: number | null;
}

// DeviceInfoDto (lib.rs:139)
export interface DeviceInfoDto {
  name: string;
  is_default: boolean;
}

// DiagnosticsPathsDto (lib.rs:146)
export interface DiagnosticsPathsDto {
  model: PathInfoDto;
  db: PathInfoDto;
  recordings: PathInfoDto;
  prompts: PathInfoDto;
}

// DiagnosticsDto (lib.rs:153)
export interface DiagnosticsDto {
  os: string;
  arch: string;
  app_version: string;
  cpal_host: string;
  input_devices: DeviceInfoDto[];
  output_devices: DeviceInfoDto[];
  paths: DiagnosticsPathsDto;
  has_anthropic_key: boolean;
  ffmpeg_ok: boolean;
  logs: string[];
}

// ── Recording start payload ───────────────────────────────────────────────────

export interface StartRecordingArgs {
  name?: string;
  source?: "mic" | "system" | "mixed";
  echo_cancel?: boolean;
}

// ── Protocol generation payload ───────────────────────────────────────────────

export interface GenerateProtocolArgs {
  meeting_id: string;
  template_name?: string;
}

// ── Job submission payload ────────────────────────────────────────────────────

export interface JobSubmitArgs {
  path: string;
  name?: string;
}
