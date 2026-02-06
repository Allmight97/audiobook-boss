export function buildJobKey(inputIndex?: number, jobId?: string): string {
  if (typeof inputIndex === "number") {
    return `idx:${inputIndex}`;
  }
  if (jobId) {
    return `job:${jobId}`;
  }
  return "default";
}
