import { invokeCmd } from "./client";
import type { GenerateProtocolArgs, ProtocolDto } from "./types";

/** mirrors: templates_list() → Vec<String> */
export function templatesList(): Promise<string[]> {
  return invokeCmd("templates_list");
}

/** mirrors: protocol_generate(meeting_id, template_name?) → ProtocolDto */
export function generateProtocol(args: GenerateProtocolArgs): Promise<ProtocolDto> {
  return invokeCmd("protocol_generate", {
    meetingId: args.meeting_id,
    templateName: args.template_name,
  });
}

/** mirrors: protocol_load(meeting_id) → Option<ProtocolDto> */
export function loadProtocol(meetingId: string): Promise<ProtocolDto | null> {
  return invokeCmd("protocol_load", { meetingId });
}
