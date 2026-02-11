import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  getCurrentCoverArt,
  initCoverArt,
  isCoverArtRemovalRequested,
  setCoverArt,
} from "../coverArt";

vi.mock("../../lib/bridge", () => ({
  bridge: {
    listen: vi.fn(),
    open: vi.fn(),
    loadCoverArtFile: vi.fn(),
    loadCoverArtFromUrl: vi.fn(),
  },
}));

describe("CoverArt island mount + clear behavior", () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="cover-art-root" class="col-span-1"></div>';
  });

  it("mounts from root and clears cover art through UI action", () => {
    initCoverArt();
    setCoverArt([0x89, 0x50, 0x4e, 0x47]);

    const clearButton = document.getElementById("cover-art-clear-btn") as HTMLButtonElement | null;
    expect(clearButton).toBeTruthy();
    expect(getCurrentCoverArt()).toEqual([0x89, 0x50, 0x4e, 0x47]);

    clearButton?.click();

    expect(getCurrentCoverArt()).toBeNull();
    expect(isCoverArtRemovalRequested()).toBe(true);
  });
});
