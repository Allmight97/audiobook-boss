import { beforeEach, describe, expect, it, vi } from "vitest";
import { stageMetadataToSelection } from "../fileList/actions";
import { setCurrentFileList, setSelectedFileIndices } from "../fileList/state";
import { getMetadataForFile, clearMetadataState } from "../metadataState";
import type { FileListInfo } from "../../types/audio";

vi.mock("../fileList/metadataPanel", () => ({
  ensureMetadataForFiles: vi.fn(async () => undefined),
  getSelectedFiles: () => [
    { path: "/a.mp3", isValid: true },
    { path: "/b.mp3", isValid: true },
  ],
}));

vi.mock("../outputPanel", () => ({
  onMetadataChange: vi.fn(),
}));

vi.mock("../metadataValidation", () => ({
  getSeriesPartValidationError: () => null,
}));

vi.mock("../metadataForm", () => ({
  readMetadataForm: () => ({ series: "Series X" }),
  resetDirtyState: vi.fn(),
}));

describe("stageMetadataToSelection", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    clearMetadataState();
    const fileList: FileListInfo = {
      files: [
        {
        path: "/a.mp3",
        size: 1,
        duration: 1,
        isValid: true,
        bitrate: 64,
        sampleRate: 44100,
        channels: 2,
      },
      {
        path: "/b.mp3",
        size: 1,
        duration: 1,
        isValid: true,
        bitrate: 64,
        sampleRate: 44100,
        channels: 2,
      },
    ],
    totalDuration: 2,
    totalSize: 2,
    validCount: 2,
    invalidCount: 0,
    };
    setCurrentFileList(fileList);
    setSelectedFileIndices([0, 1]);
  });

  it("stages changes across selected files", async () => {
    const didStage = await stageMetadataToSelection({ showStatus: false });
    expect(didStage).toBe(true);
    expect(getMetadataForFile("/a.mp3")).toMatchObject({ series: "Series X" });
    expect(getMetadataForFile("/b.mp3")).toMatchObject({ series: "Series X" });
  });
});
