import { describe, expect, beforeEach, it } from "vitest";
import { StatusPanel } from "../logic";

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
    <select id="max-concurrent-select"></select>
  `;
}

describe("StatusPanel queue snapshot", () => {
  beforeEach(() => {
    setupDom();
  });

  it("initializes queued items in order", () => {
    const panel = new StatusPanel();
    (panel as any).handleQueueSnapshot({
      items: [
        { input_index: 0, file_path: "/books/alpha.m4b" },
        { input_index: 1, file_path: "/books/beta.m4b" },
      ],
      max_concurrent: 2,
    });

    const job0 = (panel as any).jobProgress.get("idx:0");
    const job1 = (panel as any).jobProgress.get("idx:1");

    expect(job0?.status).toBe("queued");
    expect(job1?.status).toBe("queued");
    expect(job0?.label).toBe("alpha.m4b");
    expect(job1?.label).toBe("beta.m4b");
    expect((panel as any).queueOrder).toEqual(["idx:0", "idx:1"]);
  });

  it("applies queue snapshot order/labels after early progress arrives", () => {
    const panel = new StatusPanel();

    panel.updateProgress({
      input_index: 1,
      stage: "converting",
      percentage: 45,
      message: "working",
    } as any);

    expect((panel as any).queueOrder).toEqual(["idx:1"]);
    expect((panel as any).jobProgress.get("idx:1")?.status).toBe("processing");

    (panel as any).handleQueueSnapshot({
      items: [
        { input_index: 2, file_path: "/books/gamma.m4b" },
        { input_index: 1, file_path: "/books/beta.m4b" },
        { input_index: 0, file_path: "/books/alpha.m4b" },
      ],
      max_concurrent: 2,
    });

    const job2 = (panel as any).jobProgress.get("idx:2");
    const job1 = (panel as any).jobProgress.get("idx:1");
    const job0 = (panel as any).jobProgress.get("idx:0");

    expect((panel as any).queueOrder).toEqual(["idx:2", "idx:1", "idx:0"]);
    expect(job2?.label).toBe("gamma.m4b");
    expect(job1?.label).toBe("beta.m4b");
    expect(job0?.label).toBe("alpha.m4b");
    expect(job2?.status).toBe("queued");
    expect(job1?.status).toBe("queued");
    expect(job0?.status).toBe("queued");
    expect(job1?.percentage).toBe(0);
    expect(job1?.message).toBe("Queued");
  });
});
