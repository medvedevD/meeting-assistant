import { invokeCmd } from "./client";
import type { JobDto, JobSubmitArgs } from "./types";

/** mirrors: job_submit(path, name?) → JobDto */
export function jobSubmit(args: JobSubmitArgs): Promise<JobDto> {
  return invokeCmd("job_submit", { path: args.path, name: args.name });
}

/** mirrors: job_status(id) → Option<JobDto> */
export function jobStatus(id: string): Promise<JobDto | null> {
  return invokeCmd("job_status", { id });
}
