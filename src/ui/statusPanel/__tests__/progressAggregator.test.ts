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

describe("StatusPanel aggregate progress", () => {
  beforeEach(() => {
    setupDom();
  });

  it("computes simple averages across active and completed jobs", () => {
    const panel = new StatusPanel();
    const jobProgress = new Map<string, any>();
    jobProgress.set("job-1", {
      jobId: "job-1",
      stage: "converting",
      percentage: 50,
      message: "Halfway",
      lastUpdate: Date.now(),
    });
    jobProgress.set("job-2", {
      jobId: "job-2",
      stage: "completed",
      percentage: 100,
      message: "Done",
      lastUpdate: Date.now(),
    });

    (panel as any).jobProgress = jobProgress;
    const aggregate = (panel as any).calculateAggregateProgress();

    expect(aggregate.activeJobs).toBe(1);
    expect(aggregate.completedJobs).toBe(1);
    expect(aggregate.overallPercentage).toBeCloseTo(75);
  });
});
