import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

const context = vi.hoisted(() => ({
  saveMetadataToFileMock: vi.fn(),
  persistPendingDraftsMock: vi.fn(async () => false),
  getPendingEntriesMock: vi.fn(() => [] as Array<[string, Record<string, unknown>]>),
  clearPendingMock: vi.fn(),
  resetDirtyStateMock: vi.fn(),
  getCurrentFileListMock: vi.fn(() => ({
    files: [
      { path: "/books/a.m4b", isValid: true },
      { path: "/books/b.m4b", isValid: true },
    ],
  })),
  metadataSaveInProgress: false,
}));

vi.mock("../../lib/bridge", () => ({
  bridge: {
    saveMetadataToFile: context.saveMetadataToFileMock,
  },
}));

vi.mock("../fileImport", () => ({ initFileImport: vi.fn() }));
vi.mock("../outputPanel", () => ({ initOutputPanel: vi.fn() }));
vi.mock("../encoderPanel", () => ({ initEncoderPanel: vi.fn() }));
vi.mock("../coverArt", () => ({ initCoverArt: vi.fn() }));
vi.mock("../tagPreview", () => ({ initTagPreview: vi.fn() }));
vi.mock("../jobControls", () => ({ initJobControls: vi.fn() }));
vi.mock("../metadataLookup", () => ({ initMetadataLookup: vi.fn() }));
vi.mock("../statusPanel/index", () => ({
  initStatusPanel: vi.fn(),
  getStatusPanel: () => ({ isCurrentlyProcessing: false }),
}));

vi.mock("../metadataForm", () => ({
  initMetadataFormEvents: vi.fn(),
  resetDirtyState: context.resetDirtyStateMock,
}));

vi.mock("../fileList", () => ({
  getCurrentFileList: context.getCurrentFileListMock,
}));

vi.mock("../fileList/actions", () => ({
  persistPendingMetadataDraftsForCurrentSelection: context.persistPendingDraftsMock,
}));

vi.mock("../metadataState", () => ({
  getPendingMetadataEntries: () => context.getPendingEntriesMock(),
  clearPendingMetadataForFile: context.clearPendingMock,
}));

vi.mock("../metadataSaveState", () => ({
  isMetadataSaveInProgress: () => context.metadataSaveInProgress,
  setMetadataSaveInProgress: (value: boolean) => {
    context.metadataSaveInProgress = value;
  },
}));

function getSaveButton(): HTMLButtonElement {
  const button = document.getElementById("metadata-save-btn");
  if (!(button instanceof HTMLButtonElement)) {
    throw new Error("metadata-save-btn not found");
  }
  return button;
}

function getStatusText(): HTMLElement {
  const status = document.getElementById("status-text");
  if (!(status instanceof HTMLElement)) {
    throw new Error("status-text not found");
  }
  return status;
}

describe("metadata save pending flow", () => {
  beforeAll(async () => {
    document.body.innerHTML = `
      <button id="metadata-save-btn">Save All Changes</button>
      <div id="status-text">Idle</div>
    `;

    await import("../../main");
    document.dispatchEvent(new Event("DOMContentLoaded"));
  });

  beforeEach(() => {
    context.saveMetadataToFileMock.mockReset();
    context.persistPendingDraftsMock.mockReset();
    context.clearPendingMock.mockReset();
    context.resetDirtyStateMock.mockReset();
    context.getPendingEntriesMock.mockReset();
    context.metadataSaveInProgress = false;
    context.getCurrentFileListMock.mockReturnValue({
      files: [
        { path: "/books/a.m4b", isValid: true },
        { path: "/books/b.m4b", isValid: true },
      ],
    });
    getStatusText().textContent = "Idle";
  });

  it("saves all pending entries and clears pending markers on success", async () => {
    context.persistPendingDraftsMock.mockResolvedValue(true);
    context.getPendingEntriesMock.mockReturnValue([
      ["/books/a.m4b", { title: "A" }],
      ["/books/b.m4b", { title: "B" }],
    ]);
    context.saveMetadataToFileMock.mockResolvedValue(undefined);

    getSaveButton().click();

    await vi.waitFor(() => {
      expect(context.saveMetadataToFileMock).toHaveBeenCalledTimes(2);
    });
    expect(context.persistPendingDraftsMock).toHaveBeenCalledWith({ showStatus: false });
    expect(context.clearPendingMock).toHaveBeenCalledTimes(2);
    expect(context.clearPendingMock).toHaveBeenCalledWith("/books/a.m4b");
    expect(context.clearPendingMock).toHaveBeenCalledWith("/books/b.m4b");
    expect(context.resetDirtyStateMock).toHaveBeenCalledTimes(1);
    expect(getStatusText().textContent).toContain("Metadata saved (2 files)!");
  });

  it("retains failed files in pending state and surfaces partial failure summary", async () => {
    context.persistPendingDraftsMock.mockResolvedValue(true);
    context.getPendingEntriesMock.mockReturnValue([
      ["/books/a.m4b", { title: "A" }],
      ["/books/b.m4b", { title: "B" }],
    ]);
    context.saveMetadataToFileMock
      .mockResolvedValueOnce(undefined)
      .mockRejectedValueOnce(new Error("write failed"));

    getSaveButton().click();

    await vi.waitFor(() => {
      expect(context.saveMetadataToFileMock).toHaveBeenCalledTimes(2);
    });
    expect(context.clearPendingMock).toHaveBeenCalledTimes(1);
    expect(context.clearPendingMock).toHaveBeenCalledWith("/books/a.m4b");
    expect(getStatusText().textContent).toBe("Saved 1/2. Failed: b.m4b");
  });

  it("shows explicit status when there are no pending metadata changes", async () => {
    context.persistPendingDraftsMock.mockResolvedValue(false);
    context.getPendingEntriesMock.mockReturnValue([]);

    getSaveButton().click();

    await vi.waitFor(() => {
      expect(context.persistPendingDraftsMock).toHaveBeenCalledWith({
        showStatus: false,
      });
    });
    expect(context.saveMetadataToFileMock).not.toHaveBeenCalled();
    expect(getStatusText().textContent).toBe("No pending metadata changes");
  });
});
