import { describe, it, expect, beforeEach, vi } from "vitest";
import { initFileImport } from "../fileImport";

const { invokeMock, listeners } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listeners: {} as Record<string, (payload: any) => void>,
}));

vi.mock("../../lib/bridge", () => ({
  bridge: {
    listen: vi.fn((event: string, cb: any) => {
      listeners[event] = cb;
    }),
    open: vi.fn(),
    invoke: invokeMock,
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
    invokeMock.mockReset();
    document.body.innerHTML = `
      <div id="cover-art-area"></div>
      <div class="file-management-container">
        <div class="drop-zone-header" data-has-files="false"></div>
        <div class="file-list-content"></div>
      </div>
    `;
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
    const container = document.querySelector(
      ".file-management-container"
    ) as HTMLElement;
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

    initFileImport();
  });

  it("ignores drops inside cover art area", async () => {
    fireDragDrop({ x: 50, y: 50 }, ["/tmp/image.png"]);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("processes drops on file management container (header area)", async () => {
    invokeMock.mockResolvedValue({
      files: [],
      totalDuration: 0,
      totalSize: 0,
      validCount: 0,
      invalidCount: 0,
    });
    fireDragDrop({ x: 200, y: 200 }, ["/tmp/file1.mp3"]);
    expect(invokeMock).toHaveBeenCalledWith("analyze_audio_files", {
      filePaths: ["/tmp/file1.mp3"],
    });
  });

  it("processes drops on file management container (file list area)", async () => {
    invokeMock.mockResolvedValue({
      files: [],
      totalDuration: 0,
      totalSize: 0,
      validCount: 0,
      invalidCount: 0,
    });
    // Drop on file list content area (when files are present)
    fireDragDrop({ x: 200, y: 300 }, ["/tmp/file1.mp3"]);
    expect(invokeMock).toHaveBeenCalledWith("analyze_audio_files", {
      filePaths: ["/tmp/file1.mp3"],
    });
  });

  it("ignores drops outside file management container", async () => {
    invokeMock.mockResolvedValue({
      files: [],
      totalDuration: 0,
      totalSize: 0,
      validCount: 0,
      invalidCount: 0,
    });
    fireDragDrop({ x: 500, y: 500 }, ["/tmp/file1.mp3"]);
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
