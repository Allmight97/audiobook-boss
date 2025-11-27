import { FileListInfo } from "../types/audio";
import { AudiobookMetadata } from "../types/metadata";
import { ProcessingProgressEvent, EVENTS, STAGES } from "../types/events";

// Mock Data
const MOCK_FILE_LIST: FileListInfo = {
  files: [
    {
      path: "/mock/path/chapter1.mp3",
      size: 15 * 1024 * 1024, // 15MB
      duration: 300, // 5 minutes
      isValid: true,
      bitrate: 64,
      sampleRate: 44100,
      channels: 1,
    },
    {
      path: "/mock/path/chapter2.mp3",
      size: 20 * 1024 * 1024, // 20MB
      duration: 400, // 6:40 minutes
      isValid: true,
      bitrate: 64,
      sampleRate: 44100,
      channels: 1,
    },
  ],
  totalDuration: 700,
  totalSize: 35 * 1024 * 1024,
  validCount: 2,
  invalidCount: 0,
};

const MOCK_METADATA: AudiobookMetadata = {
  title: "Mock Audiobook Title",
  author: "Mock Author",
  album: "Mock Album",
  narrator: "Mock Narrator",
  year: 2023,
  genre: "Audiobook",
  description: "This is a mock description for testing purposes.",
  series: "Mock Series",
  cover_art: undefined,
};

// Event Simulation Helpers
type EventHandler = (event: any) => void;
const listeners: Map<string, Set<EventHandler>> = new Map();

export function mockListen(
  event: string,
  handler: EventHandler
): Promise<() => void> {
  console.log(`[Bridge Mock] Listening for event: ${event}`);
  if (!listeners.has(event)) {
    listeners.set(event, new Set());
  }
  listeners.get(event)?.add(handler);

  return Promise.resolve(() => {
    console.log(`[Bridge Mock] Unlistening event: ${event}`);
    listeners.get(event)?.delete(handler);
  });
}

function emitEvent(event: string, payload: any) {
  const handlers = listeners.get(event);
  if (handlers) {
    handlers.forEach((h) => h({ payload }));
  }
}

// Mock Command Implementations
export async function mockInvoke<T>(cmd: string, args?: any): Promise<T> {
  console.log(`[Bridge Mock] Invoke: ${cmd}`, args);

  // Simulate network delay
  await new Promise((resolve) => setTimeout(resolve, 500));

  switch (cmd) {
    case "analyze_audio_files":
      return MOCK_FILE_LIST as unknown as T;

    case "read_audio_metadata":
      return MOCK_METADATA as unknown as T;

    case "list_available_encoders":
      return {
        fdk_available: false,
        aac_at_available: true,
        opus_available: true,
        native_aac_available: true,
      } as unknown as T;

    case "process_audiobook_files_v2":
      // Start a simulated progress loop
      simulateProcessing();
      return { message: "Processing started (mock)" } as unknown as T;

    case "cancel_processing":
      emitEvent(EVENTS.PROGRESS, {
        stage: STAGES.cancelled,
        percentage: 0,
        message: "Cancelled by user",
        current_file: "",
        eta_seconds: 0,
      } as ProcessingProgressEvent);
      return undefined as unknown as T;

    case "load_cover_art_file":
      // Return a small 1x1 transparent pixel as mock cover art
      return [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00,
        0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
      ] as unknown as T;

    default:
      console.warn(`[Bridge Mock] Unhandled command: ${cmd}`);
      return undefined as unknown as T;
  }
}

export async function mockOpen(
  options?: any
): Promise<string | string[] | null> {
  console.log(`[Bridge Mock] Open Dialog`, options);
  if (options?.multiple) {
    return ["/mock/path/file1.mp3", "/mock/path/file2.mp3"];
  }
  return "/mock/path/selected_file.mp3";
}

export async function mockOpenExternal(path: string): Promise<void> {
  console.log(`[Bridge Mock] Open External: ${path}`);
  alert(`[Mock] Opening external path: ${path}`);
}

// Simulation Logic
function simulateProcessing() {
  let progress = 0;
  const interval = setInterval(() => {
    progress += 10;

    if (progress > 100) {
      clearInterval(interval);
      emitEvent(EVENTS.PROGRESS, {
        stage: STAGES.completed,
        percentage: 100,
        message: "Processing Complete",
        current_file: "",
        eta_seconds: 0,
      } as ProcessingProgressEvent);
    } else {
      emitEvent(EVENTS.PROGRESS, {
        stage: STAGES.converting,
        percentage: progress,
        message: `Processing... ${progress}%`,
        current_file: "mock_file.mp3",
        eta_seconds: (100 - progress) / 10,
      } as ProcessingProgressEvent);
    }
  }, 500);
}
