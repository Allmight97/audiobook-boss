import { beforeEach, describe, expect, it, vi } from "vitest";

const { initStatusPanelLogicMock } = vi.hoisted(() => ({
  initStatusPanelLogicMock: vi.fn(() => ({ isCurrentlyProcessing: false })),
}));

vi.mock("../logic", () => ({
  StatusPanel: class {},
  getStatusPanel: vi.fn(() => null),
  initStatusPanel: initStatusPanelLogicMock,
}));

import { initStatusPanel } from "../index";

describe("StatusPanel island mount", () => {
  beforeEach(() => {
    initStatusPanelLogicMock.mockClear();
    document.body.innerHTML = '<div id="status-panel-root"></div>';
  });

  it("mounts status panel markup and delegates initialization", () => {
    initStatusPanel();

    expect(document.querySelector(".panel.status-panel")).toBeTruthy();
    const requiredIds = [
      "progress-bar",
      "percentage-processed",
      "status-text",
      "step-text",
      "concurrency-status",
      "process-button",
      "cancel-all-button",
      "job-list",
    ];
    requiredIds.forEach((id) => {
      expect(document.getElementById(id)).toBeTruthy();
    });
    expect(initStatusPanelLogicMock).toHaveBeenCalledTimes(1);
  });
});
