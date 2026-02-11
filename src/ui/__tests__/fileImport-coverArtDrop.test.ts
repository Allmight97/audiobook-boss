import { describe, it, expect, beforeEach, vi } from "vitest";
import { initFileImport } from "../fileImport";

const { analyzeAudioFilesMock, listeners } = vi.hoisted(() => ({
  analyzeAudioFilesMock: vi.fn(),
  listeners: {} as Record<string, (payload: any) => void>,
}));

vi.mock("../../lib/bridge", () => ({
  bridge: {
    listen: vi.fn((event: string, cb: any) => {
      listeners[event] = cb;
    }),
    open: vi.fn(),
    analyzeAudioFiles: analyzeAudioFilesMock,
  },
}));

function fireDragDrop(position: { x: number; y: number }, paths: string[]) {
  const handler = listeners["tauri://drag-drop"];
  if (handler) {
    handler({ payload: { position, paths } });
  }
}

describe("File import drop vs cover art drop isolation", () => {
  beforeEach(() => {
    analyzeAudioFilesMock.mockReset();
    document.body.innerHTML = `
      <div id="cover-art-area"></div>
      <div id="file-import-root"></div>
    `;
    initFileImport();

    const dropZone = document.querySelector(".drop-zone-header") as HTMLElement | null;
    if (!dropZone) {
      throw new Error("Expected file import island to render drop zone");
    }

    const container = document.querySelector(
      ".file-management-container"
    ) as HTMLElement | null;
    if (!container) {
      throw new Error("Expected file import island to render file management container");
    }

    // Mock cover art bounds
    const cover = document.getElementById("cover-art-area") as HTMLElement;
    cover.getBoundingClientRect = () =>
      ({
        left: 0,
        right: 100,
        top: 0,
        bottom: 100,
        width: 100,
        height: 100,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      } as DOMRect);

    // Mock file management container bounds (entire drop area)
    container.getBoundingClientRect = () =>
      ({
        left: 150,
        right: 400,
        top: 150,
        bottom: 350,
        width: 250,
        height: 200,
        x: 150,
        y: 150,
        toJSON: () => ({}),
      } as DOMRect);
  });

  it("ignores drops inside cover art area", async () => {
    fireDragDrop({ x: 50, y: 50 }, ["/tmp/image.png"]);
    expect(analyzeAudioFilesMock).not.toHaveBeenCalled();
  });

  it("processes drops on file management container (header area)", async () => {
    analyzeAudioFilesMock.mockResolvedValue({
      files: [],
      totalDuration: 0,
      totalSize: 0,
      validCount: 0,
      invalidCount: 0,
    });
    fireDragDrop({ x: 200, y: 200 }, ["/tmp/file1.mp3"]);
    expect(analyzeAudioFilesMock).toHaveBeenCalledWith(["/tmp/file1.mp3"]);
  });

  it("processes drops on file management container (file list area)", async () => {
    analyzeAudioFilesMock.mockResolvedValue({
      files: [],
      totalDuration: 0,
      totalSize: 0,
      validCount: 0,
      invalidCount: 0,
    });
    // Drop on file list content area (when files are present)
    fireDragDrop({ x: 200, y: 300 }, ["/tmp/file1.mp3"]);
    expect(analyzeAudioFilesMock).toHaveBeenCalledWith(["/tmp/file1.mp3"]);
  });

  it("ignores drops outside file management container", async () => {
    analyzeAudioFilesMock.mockResolvedValue({
      files: [],
      totalDuration: 0,
      totalSize: 0,
      validCount: 0,
      invalidCount: 0,
    });
    fireDragDrop({ x: 500, y: 500 }, ["/tmp/file1.mp3"]);
    expect(analyzeAudioFilesMock).not.toHaveBeenCalled();
  });
});
