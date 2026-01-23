import { describe, it, expect, beforeEach } from "vitest";
import { StatusPanel } from "./logic";

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
    <div id="output-dir-text"></div>
  `;
}

describe("StatusPanel progress throttling", () => {
  beforeEach(() => {
    setupDom();
  });

  it("throttles rapid non-terminal progress events", async () => {
    const panel = new StatusPanel();

    const evt = {
      job_id: "job-1",
      stage: "converting",
      percentage: 10,
      message: "ten",
    } as any;

    panel.updateProgress(evt);
    let job = (panel as any).jobProgress.get("job:job-1");
    expect(job?.percentage).toBe(10);

    const evt2 = { ...evt, percentage: 20, message: "twenty" };
    panel.updateProgress(evt2);
    job = (panel as any).jobProgress.get("job:job-1");
    expect(job?.percentage).toBe(10);

    await new Promise((resolve) => setTimeout(resolve, 550));
    const evt3 = { ...evt, percentage: 30, message: "thirty" };
    panel.updateProgress(evt3);
    const updated = (panel as any).jobProgress.get("job:job-1");
    expect(updated?.percentage).toBe(30);
  });

  it("does not throttle different jobs within the same window", () => {
    const panel = new StatusPanel();

    const evt1 = {
      job_id: "job-1",
      stage: "converting",
      percentage: 10,
      message: "ten",
    } as any;

    const evt2 = {
      job_id: "job-2",
      stage: "converting",
      percentage: 20,
      message: "twenty",
    } as any;

    panel.updateProgress(evt1);
    panel.updateProgress(evt2);

    const job1 = (panel as any).jobProgress.get("job:job-1");
    const job2 = (panel as any).jobProgress.get("job:job-2");
    expect(job1?.percentage).toBe(10);
    expect(job2?.percentage).toBe(20);
  });
});
