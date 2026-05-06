import { invokeCmd } from "./client";
import type { GenerateProtocolArgs, ProtocolDto } from "./types";

/** mirrors: templates_list() → Vec<String> */
export function templatesList(): Promise<string[]> {
  return invokeCmd("templates_list");
}

/** mirrors: protocol_generate(meeting_id, template_name?) → ProtocolDto */
export function generateProtocol(args: GenerateProtocolArgs): Promise<ProtocolDto> {
  return invokeCmd("protocol_generate", {
    meeting_id: args.meeting_id,
    template_name: args.template_name,
  });
}
