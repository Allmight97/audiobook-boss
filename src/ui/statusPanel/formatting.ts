import type { AggregateProgress, JobProgress, ProcessingStatus } from "./state";

export function formatStatusDisplayText(
  stage: ProcessingStatus["stage"]
): string {
  switch (stage) {
    case "idle":
      return "Idle";
    case "analyzing":
      return "Analyzing";
    case "converting":
      return "Converting";
    case "writing":
      return "Writing Metadata";
    case "completed":
      return "Completed";
    case "cancelled":
      return "Cancelled";
    case "failed":
      return "Failed";
    default:
      return "Processing";
  }
}

export function formatAggregateMessage(
  jobProgress: Map<string, JobProgress>,
  aggregate: AggregateProgress
): string {
  if (aggregate.activeJobs > 1) {
    const queuedSuffix =
      aggregate.queuedJobs > 0 ? `, ${aggregate.queuedJobs} queued` : "";
    const completedSuffix =
      aggregate.completedJobs > 0 ? `, ${aggregate.completedJobs} completed` : "";
    return `Processing ${aggregate.activeJobs} files${queuedSuffix}${completedSuffix}`;
  }
  if (aggregate.activeJobs === 1) {
    const activeJob = Array.from(jobProgress.values()).find(
      (job) => job.status === "processing"
    );
    return activeJob?.message ?? "Processing...";
  }
  if (aggregate.queuedJobs > 0) {
    return `Queued ${aggregate.queuedJobs} file${aggregate.queuedJobs === 1 ? "" : "s"}`;
  }
  return "Ready to process audiobook";
}

export function convertBytesToDataUrl(bytes: number[]): string {
  const uint8Array = new Uint8Array(bytes);

  let mimeType = "image/jpeg"; // default fallback
  if (uint8Array.length >= 4) {
    // PNG: 89 50 4E 47
    if (
      uint8Array[0] === 0x89 &&
      uint8Array[1] === 0x50 &&
      uint8Array[2] === 0x4e &&
      uint8Array[3] === 0x47
    ) {
      mimeType = "image/png";
    }
    // JPEG: FF D8 FF
    else if (
      uint8Array[0] === 0xff &&
      uint8Array[1] === 0xd8 &&
      uint8Array[2] === 0xff
    ) {
      mimeType = "image/jpeg";
    }
    // WebP: 52 49 46 46 ... 57 45 42 50
    else if (
      uint8Array[0] === 0x52 &&
      uint8Array[1] === 0x49 &&
      uint8Array[2] === 0x46 &&
      uint8Array[3] === 0x46
    ) {
      mimeType = "image/webp";
    }
  }

  let binary = "";
  uint8Array.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  const base64 = btoa(binary);

  return `data:${mimeType};base64,${base64}`;
}

export function extractFilenameFromProgress(label: string): string | null {
  const trimmed = label.trim();
  if (!trimmed) return null;
  const match = trimmed.match(/^(.*?) \(\d+\/\d+\)$/);
  if (match && match[1]) {
    return match[1].trim();
  }
  return trimmed;
}

function splitPathSegments(path: string): string[] {
  return path.split(/[\\/]/).filter(Boolean);
}

export function buildQueueLabels(paths: string[]): string[] {
  const segmentsList = paths.map(splitPathSegments);
  const basenames = segmentsList.map(
    (segments) => segments[segments.length - 1] ?? ""
  );
  const nameCounts = basenames.reduce<Record<string, number>>((acc, name) => {
    acc[name] = (acc[name] ?? 0) + 1;
    return acc;
  }, {});

  let labels = basenames.map((name, index) => {
    if (nameCounts[name] === 1) return name;
    const segments = segmentsList[index];
    const parent = segments.length > 1 ? segments[segments.length - 2] : "";
    return parent ? `${name} (${parent})` : name;
  });

  const labelCounts = labels.reduce<Record<string, number>>((acc, label) => {
    acc[label] = (acc[label] ?? 0) + 1;
    return acc;
  }, {});

  labels = labels.map((label, index) => {
    if (labelCounts[label] === 1) return label;
    const segments = segmentsList[index];
    const tail = segments.slice(-2).join("/");
    return tail || label;
  });

  return labels;
}
