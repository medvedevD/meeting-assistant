import { invokeCmd } from "./client";
import type { MeetingDto } from "./types";

/** mirrors: meetings_list() → Vec<MeetingDto> */
export function listMeetings(): Promise<MeetingDto[]> {
  return invokeCmd("meetings_list");
}
