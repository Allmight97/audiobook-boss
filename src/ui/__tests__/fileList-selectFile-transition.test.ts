import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FileListInfo } from "../../types/audio";
import { setCurrentFileList, setSelectedIndex } from "../fileList/state";

const context = vi.hoisted(() => ({
  readMetadataFormMock: vi.fn(() => ({ title: "Persisted Title" })),
  setMetadataForFileMock: vi.fn(),
  getSelectedFilesMock: vi.fn(),
  handleSelectionMock: vi.fn(() => ({ changed: true })),
  showSingleSelectionMock: vi.fn(),
}));

vi.mock("../metadataForm", () => ({
  readMetadataForm: context.readMetadataFormMock,
  resetDirtyState: vi.fn(),
}));

vi.mock("../metadataState", () => ({
  clearMetadataState: vi.fn(),
  getMetadataForFile: vi.fn(),
  removeMetadataForFile: vi.fn(),
  setMetadataForFile: context.setMetadataForFileMock,
}));

vi.mock("../outputPanel", () => ({
  onFileListChange: vi.fn(),
  onMetadataChange: vi.fn(),
}));

vi.mock("../metadataValidation", () => ({
  getSeriesPartValidationError: vi.fn(() => null),
  getSubseriesPartValidationError: vi.fn(() => null),
}));

vi.mock("../fileList/dom", () => ({
  updateFileListDOM: vi.fn(),
  updateTotalStats: vi.fn(),
  updateSelection: vi.fn(),
  updateSortButtonText: vi.fn(),
  updateButtonVisibility: vi.fn(),
  showEmptyState: vi.fn(),
  setOrderLockNotice: vi.fn(),
}));

vi.mock("../fileList/events", () => ({
  initFileListEvents: vi.fn(),
  setupDragStartHandlers: vi.fn(),
}));

vi.mock("../fileList/selection", () => ({
  clearSelection: vi.fn(() => true),
  handleSelection: context.handleSelectionMock,
  reindexSelectionAfterMove: vi.fn(),
  reindexSelectionAfterRemoval: vi.fn(),
  selectAllFiles: vi.fn(() => true),
  swapSelectionIndices: vi.fn(),
}));

vi.mock("../fileList/metadataPanel", () => ({
  autoUpdateCoverArtFromFirstValidFile: vi.fn(async () => undefined),
  clearSelectionPanels: vi.fn(),
  ensureMetadataForFiles: vi.fn(async () => undefined),
  getSelectedFiles: context.getSelectedFilesMock,
  showMultiSelection: vi.fn(async () => undefined),
  showSingleSelection: context.showSingleSelectionMock,
}));

describe("selectFile transition options", () => {
  beforeEach(() => {
    const fileList: FileListInfo = {
      files: [
        {
          path: "/books/alpha.m4b",
          size: 1,
          duration: 1,
          isValid: true,
          bitrate: 64,
          sampleRate: 44100,
          channels: 2,
        },
        {
          path: "/books/beta.m4b",
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
    setSelectedIndex(0);

    context.readMetadataFormMock.mockClear();
    context.setMetadataForFileMock.mockClear();
    context.handleSelectionMock.mockClear();
    context.showSingleSelectionMock.mockClear();
    context.getSelectedFilesMock.mockReset();
    context.getSelectedFilesMock.mockReturnValue([
      {
        path: "/books/alpha.m4b",
        isValid: true,
      },
    ]);
  });

  it("skips previous-file autosave for queue-managed transitions", async () => {
    const { selectFile } = await import("../fileList/actions");
    await selectFile(
      1,
      { multi: false, range: false },
      { skipPersistPrevious: true }
    );

    expect(context.readMetadataFormMock).not.toHaveBeenCalled();
    expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
  });

  it("preserves default autosave behavior when transition option is omitted", async () => {
    const { selectFile } = await import("../fileList/actions");
    await selectFile(1, { multi: false, range: false });

    expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: "single" });
    expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
      "/books/alpha.m4b",
      { title: "Persisted Title" }
    );
  });
});
