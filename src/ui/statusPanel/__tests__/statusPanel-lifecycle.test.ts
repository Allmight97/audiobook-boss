import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bridge } from "../../../lib/bridge";
import { STAGES } from "../../../types/events";
import * as dom from "../dom";
import { StatusPanel } from "../logic";

const listenerState = vi.hoisted(() => {
  const progressCallbacks = new Set<(event: any) => void>();
  const queueCallbacks = new Set<(event: any) => void>();
  const progressUnlisteners: Array<ReturnType<typeof vi.fn>> = [];
  const queueUnlisteners: Array<ReturnType<typeof vi.fn>> = [];

  return {
    progressCallbacks,
    queueCallbacks,
    progressUnlisteners,
    queueUnlisteners,
    listenForProgressEventsMock: vi.fn(async (handler: (event: any) => void) => {
      progressCallbacks.add(handler);
      const unlisten = vi.fn(() => {
        progressCallbacks.delete(handler);
      });
      progressUnlisteners.push(unlisten);
      return unlisten;
    }),
    listenForQueueEventsMock: vi.fn(async (handler: (event: any) => void) => {
      queueCallbacks.add(handler);
      const unlisten = vi.fn(() => {
        queueCallbacks.delete(handler);
      });
      queueUnlisteners.push(unlisten);
      return unlisten;
    }),
  };
});

vi.mock("../events", () => ({
  bindStatusPanelDomEvents: vi.fn(),
  listenForProgressEvents: listenerState.listenForProgressEventsMock,
  listenForQueueEvents: listenerState.listenForQueueEventsMock,
}));

function setupDom() {
  document.body.innerHTML = `
    <div id="progress-bar"></div>
    <div id="percentage-processed"></div>
    <div id="status-text"></div>
    <div id="step-text"></div>
    <div id="concurrency-status"></div>
    <button id="process-button"></button>
    <button id="cancel-all-button"></button>
    <div class="art-thumbnail"></div>
    <div id="job-list"></div>
    <input id="merge-mode-toggle" type="checkbox" />
    <select id="max-concurrent-select"></select>
  `;
}

function seedDisabledControls() {
  const mergeToggle = document.getElementById("merge-mode-toggle") as HTMLInputElement;
  const maxConcurrent = document.getElementById(
    "max-concurrent-select"
  ) as HTMLSelectElement;
  mergeToggle.disabled = true;
  mergeToggle.style.opacity = "0.5";
  maxConcurrent.disabled = true;
  maxConcurrent.style.opacity = "0.5";
}

function assertControlsEnabled() {
  const mergeToggle = document.getElementById("merge-mode-toggle") as HTMLInputElement;
  const maxConcurrent = document.getElementById(
    "max-concurrent-select"
  ) as HTMLSelectElement;
  expect(mergeToggle.disabled).toBe(false);
  expect(maxConcurrent.disabled).toBe(false);
  expect(mergeToggle.style.opacity).toBe("1");
  expect(maxConcurrent.style.opacity).toBe("1");
}

function emitProgressToActiveListeners(event: any) {
  listenerState.progressCallbacks.forEach((callback) => callback(event));
}

describe("StatusPanel lifecycle", () => {
  beforeEach(() => {
    setupDom();
    dom.resetStatusPanelDomCache();
    vi.useFakeTimers();

    listenerState.progressCallbacks.clear();
    listenerState.queueCallbacks.clear();
    listenerState.progressUnlisteners.length = 0;
    listenerState.queueUnlisteners.length = 0;
    listenerState.listenForProgressEventsMock.mockClear();
    listenerState.listenForQueueEventsMock.mockClear();
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("disables cancel-all while cancel request is in flight and restores on success", async () => {
    const panel = new StatusPanel();
    const cancelButton = document.getElementById(
      "cancel-all-button"
    ) as HTMLButtonElement;

    let resolveCancel!: () => void;
    const inFlightCancel = new Promise<void>((resolve) => {
      resolveCancel = resolve;
    });
    const cancelSpy = vi
      .spyOn(bridge, "cancelProcessing")
      .mockReturnValue(inFlightCancel as any);

    const cancelRequest = (panel as any).handleCancelAll();
    expect(cancelSpy).toHaveBeenCalledTimes(1);
    expect(cancelButton.disabled).toBe(true);

    resolveCancel();
    await cancelRequest;

    expect(cancelButton.disabled).toBe(false);
    expect(panel.getCurrentStatus().message).toBe("Cancellation requested…");
    expect((document.getElementById("step-text") as HTMLElement).textContent).toContain(
      "Cancellation requested…"
    );
  });

  it("restores cancel-all enabled state and surfaces explicit error on cancel failure", async () => {
    const panel = new StatusPanel();
    const cancelButton = document.getElementById(
      "cancel-all-button"
    ) as HTMLButtonElement;
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);
    vi.spyOn(bridge, "cancelProcessing").mockRejectedValue(
      new Error("bridge cancellation failed")
    );

    await (panel as any).handleCancelAll();

    expect(cancelButton.disabled).toBe(false);
    expect((document.getElementById("step-text") as HTMLElement).textContent).toBe(
      "Error: Failed to cancel processing. Please try again."
    );
    expect(consoleErrorSpy).toHaveBeenCalled();
  });

  it.each([
    {
      name: "all completed",
      terminalStages: [STAGES.completed, STAGES.completed] as const,
      expectedMethod: "showSuccess" as const,
      expectedMessage: "Audiobook created successfully!",
    },
    {
      name: "failed present",
      terminalStages: [STAGES.failed, STAGES.completed] as const,
      expectedMethod: "showError" as const,
      expectedMessage: "One or more files failed to process.",
    },
    {
      name: "cancelled present",
      terminalStages: [STAGES.cancelled, STAGES.completed] as const,
      expectedMethod: "showInfo" as const,
      expectedMessage: "Processing was cancelled.",
    },
  ])(
    "applies batch terminal lifecycle reset after 2s when %s",
    ({ terminalStages, expectedMethod, expectedMessage }) => {
      const panel = new StatusPanel();
      seedDisabledControls();

      const progressUnlisten = vi.fn();
      const queueUnlisten = vi.fn();
      (panel as any).progressUnlisten = progressUnlisten;
      (panel as any).queueUnlisten = queueUnlisten;

      const showSuccessSpy = vi.spyOn(dom, "showSuccess");
      const showErrorSpy = vi.spyOn(dom, "showError");
      const showInfoSpy = vi.spyOn(dom, "showInfo");

      (panel as any).handleQueueSnapshot({
        items: [
          { input_index: 0, file_path: "/books/alpha.m4b" },
          { input_index: 1, file_path: "/books/beta.m4b" },
        ],
        max_concurrent: 2,
      });

      panel.updateProgress({
        input_index: 0,
        stage: terminalStages[0],
        percentage: 100,
        message: "terminal-0",
      } as any);
      panel.updateProgress({
        input_index: 1,
        stage: terminalStages[1],
        percentage: 100,
        message: "terminal-1",
      } as any);

      expect((panel as any).batchCompletionTimeout).toBeDefined();
      vi.advanceTimersByTime(1999);
      expect(progressUnlisten).not.toHaveBeenCalled();
      expect(queueUnlisten).not.toHaveBeenCalled();

      vi.advanceTimersByTime(1);

      const expectedSpy =
        expectedMethod === "showSuccess"
          ? showSuccessSpy
          : expectedMethod === "showError"
            ? showErrorSpy
            : showInfoSpy;
      expect(expectedSpy).toHaveBeenCalledWith(expectedMessage);
      expect(progressUnlisten).toHaveBeenCalledTimes(1);
      expect(queueUnlisten).toHaveBeenCalledTimes(1);
      assertControlsEnabled();
      expect(panel.getCurrentStatus()).toEqual({
        stage: "idle",
        percentage: 0,
        message: "Ready to process audiobook",
      });
    }
  );

  it.each([
    {
      name: "completed",
      stage: STAGES.completed,
      message: "Audiobook created successfully!",
      method: "showSuccess" as const,
    },
    {
      name: "failed",
      stage: STAGES.failed,
      message: "Encoder failed",
      method: "showError" as const,
    },
    {
      name: "cancelled",
      stage: STAGES.cancelled,
      message: "Processing was cancelled.",
      method: "showInfo" as const,
    },
  ])(
    "applies single-job terminal lifecycle reset after 2s when %s",
    ({ stage, message, method }) => {
      const panel = new StatusPanel();
      seedDisabledControls();

      const progressUnlisten = vi.fn();
      const queueUnlisten = vi.fn();
      (panel as any).progressUnlisten = progressUnlisten;
      (panel as any).queueUnlisten = queueUnlisten;

      const showSuccessSpy = vi.spyOn(dom, "showSuccess");
      const showErrorSpy = vi.spyOn(dom, "showError");
      const showInfoSpy = vi.spyOn(dom, "showInfo");

      panel.updateProgress({
        job_id: "job-1",
        stage,
        percentage: 100,
        message,
      } as any);

      vi.advanceTimersByTime(1999);
      expect(progressUnlisten).not.toHaveBeenCalled();
      expect(queueUnlisten).not.toHaveBeenCalled();

      vi.advanceTimersByTime(1);

      const expectedSpy =
        method === "showSuccess"
          ? showSuccessSpy
          : method === "showError"
            ? showErrorSpy
            : showInfoSpy;
      expect(expectedSpy).toHaveBeenCalledWith(message);
      expect(progressUnlisten).toHaveBeenCalledTimes(1);
      expect(queueUnlisten).toHaveBeenCalledTimes(1);
      assertControlsEnabled();
      expect(panel.getCurrentStatus()).toEqual({
        stage: "idle",
        percentage: 0,
        message: "Ready to process audiobook",
      });
    }
  );

  it("cleans up listeners on reset and restarts without duplicate active handlers", async () => {
    const panel = new StatusPanel();
    const updateProgressSpy = vi.spyOn(panel, "updateProgress");

    await (panel as any).startProgressListener();
    expect(listenerState.progressCallbacks.size).toBe(1);
    expect(listenerState.queueCallbacks.size).toBe(1);

    (panel as any).resetToIdle();
    expect(listenerState.progressUnlisteners[0]).toHaveBeenCalledTimes(1);
    expect(listenerState.queueUnlisteners[0]).toHaveBeenCalledTimes(1);
    expect(listenerState.progressCallbacks.size).toBe(0);
    expect(listenerState.queueCallbacks.size).toBe(0);

    await (panel as any).startProgressListener();
    expect(listenerState.listenForProgressEventsMock).toHaveBeenCalledTimes(2);
    expect(listenerState.listenForQueueEventsMock).toHaveBeenCalledTimes(2);
    expect(listenerState.progressCallbacks.size).toBe(1);
    expect(listenerState.queueCallbacks.size).toBe(1);

    emitProgressToActiveListeners({
      job_id: "job-123",
      stage: "converting",
      percentage: 35,
      message: "Converting",
    });

    expect(updateProgressSpy).toHaveBeenCalledTimes(1);
    (panel as any).resetToIdle();
    expect(listenerState.progressUnlisteners[1]).toHaveBeenCalledTimes(1);
    expect(listenerState.queueUnlisteners[1]).toHaveBeenCalledTimes(1);
  });
});
