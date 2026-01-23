import { describe, expect, it } from "vitest";
import { STAGES } from "../../../types/events";
import type {
  ProcessingProgressEvent,
  ProcessingQueueEvent,
} from "../../../types/events";
import type { JobProgress } from "../state";
import {
  areAllJobsTerminal,
  buildJobKey,
  hasCancelledJobs,
  hasFailedJobs,
  processProgressUpdate,
  processQueueSnapshot,
  shouldThrottleUpdate,
} from "../jobState";

describe("jobState", () => {
  describe("buildJobKey", () => {
    it("builds key from input index", () => {
      expect(buildJobKey(0, undefined)).toBe("idx:0");
      expect(buildJobKey(5, undefined)).toBe("idx:5");
    });

    it("builds key from job ID when no input index", () => {
      expect(buildJobKey(undefined, "job-123")).toBe("job:job-123");
    });

    it("prefers input index over job ID", () => {
      expect(buildJobKey(3, "job-456")).toBe("idx:3");
    });

    it("returns default when neither provided", () => {
      expect(buildJobKey(undefined, undefined)).toBe("default");
    });
  });

  describe("processQueueSnapshot", () => {
    it("processes queue snapshot into jobs and order", () => {
      const event: ProcessingQueueEvent = {
        items: [
          { input_index: 0, file_path: "/path/to/file1.m4b" },
          { input_index: 1, file_path: "/path/to/file2.m4b" },
        ],
        max_concurrent: 2,
      };

      const now = Date.now();
      const result = processQueueSnapshot(event, now);

      expect(result.jobs.size).toBe(2);
      expect(result.order).toEqual(["idx:0", "idx:1"]);

      const job0 = result.jobs.get("idx:0");
      expect(job0).toMatchObject({
        inputIndex: 0,
        label: "file1.m4b",
        status: "queued",
        percentage: 0,
        message: "Queued",
        lastUpdate: now,
      });
    });

    it("handles empty queue", () => {
      const event: ProcessingQueueEvent = {
        items: [],
        max_concurrent: 2,
      };

      const result = processQueueSnapshot(event);

      expect(result.jobs.size).toBe(0);
      expect(result.order).toEqual([]);
    });

    it("disambiguates duplicate file names", () => {
      const event: ProcessingQueueEvent = {
        items: [
          { input_index: 0, file_path: "/books/scifi/book.m4b" },
          { input_index: 1, file_path: "/books/fantasy/book.m4b" },
        ],
        max_concurrent: 2,
      };

      const result = processQueueSnapshot(event);

      const job0 = result.jobs.get("idx:0");
      const job1 = result.jobs.get("idx:1");

      expect(job0?.label).toBe("scifi/book.m4b");
      expect(job1?.label).toBe("fantasy/book.m4b");
    });
  });

  describe("processProgressUpdate", () => {
    it("creates new job from progress event", () => {
      const event: ProcessingProgressEvent = {
        stage: STAGES.converting,
        percentage: 45.5,
        message: "Converting audio",
        job_id: "job-123",
        input_index: 0,
      };

      const now = Date.now();
      const result = processProgressUpdate(
        event,
        undefined,
        "fallback-label",
        now
      );

      expect(result.isTerminal).toBe(false);
      expect(result.job).toMatchObject({
        jobId: "job-123",
        inputIndex: 0,
        label: "fallback-label",
        status: "processing",
        stage: STAGES.converting,
        percentage: 45.5,
        message: "Converting audio",
        lastUpdate: now,
      });
    });

    it("updates existing job", () => {
      const existing: JobProgress = {
        inputIndex: 0,
        label: "existing-label",
        status: "processing",
        stage: STAGES.analyzing,
        percentage: 10,
        message: "Analyzing",
        lastUpdate: Date.now() - 1000,
      };

      const event: ProcessingProgressEvent = {
        stage: STAGES.converting,
        percentage: 60,
        message: "Converting",
        input_index: 0,
      };

      const result = processProgressUpdate(event, existing, "fallback");

      expect(result.job.label).toBe("existing-label"); // Preserves existing label
      expect(result.job.percentage).toBe(60);
      expect(result.job.stage).toBe(STAGES.converting);
    });

    it("marks terminal stages correctly", () => {
      const testCases = [
        { stage: STAGES.completed, expectedStatus: "completed" },
        { stage: STAGES.failed, expectedStatus: "failed" },
        { stage: STAGES.cancelled, expectedStatus: "cancelled" },
      ];

      testCases.forEach(({ stage, expectedStatus }) => {
        const event: ProcessingProgressEvent = {
          stage,
          percentage: 100,
          message: "Done",
          input_index: 0,
        };

        const result = processProgressUpdate(event, undefined, "label");

        expect(result.isTerminal).toBe(true);
        expect(result.job.status).toBe(expectedStatus);
      });
    });

    it("rounds percentage to one decimal place", () => {
      const event: ProcessingProgressEvent = {
        stage: STAGES.converting,
        percentage: 45.567,
        message: "Converting",
        input_index: 0,
      };

      const result = processProgressUpdate(event, undefined, "label");

      expect(result.job.percentage).toBe(45.6);
    });
  });

  describe("shouldThrottleUpdate", () => {
    it("never throttles terminal events", () => {
      const lastRenderTimes = new Map([["idx:0", Date.now() - 100]]);
      const now = Date.now();

      expect(
        shouldThrottleUpdate("idx:0", true, lastRenderTimes, now, 500)
      ).toBe(false);
    });

    it("throttles non-terminal updates within window", () => {
      const lastRenderTimes = new Map([["idx:0", Date.now() - 200]]);
      const now = Date.now();

      expect(
        shouldThrottleUpdate("idx:0", false, lastRenderTimes, now, 500)
      ).toBe(true);
    });

    it("allows non-terminal updates after throttle window", () => {
      const lastRenderTimes = new Map([["idx:0", Date.now() - 600]]);
      const now = Date.now();

      expect(
        shouldThrottleUpdate("idx:0", false, lastRenderTimes, now, 500)
      ).toBe(false);
    });

    it("allows updates for jobs never rendered", () => {
      const lastRenderTimes = new Map<string, number>();
      const now = Date.now();

      expect(
        shouldThrottleUpdate("idx:0", false, lastRenderTimes, now, 500)
      ).toBe(false);
    });
  });

  describe("areAllJobsTerminal", () => {
    it("returns false for empty queue", () => {
      const jobs = new Map<string, JobProgress>();
      expect(areAllJobsTerminal(jobs, [])).toBe(false);
    });

    it("returns true when all jobs completed", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "completed",
            percentage: 100,
            message: "Done",
            lastUpdate: Date.now(),
          },
        ],
        [
          "idx:1",
          {
            inputIndex: 1,
            label: "file2",
            status: "completed",
            percentage: 100,
            message: "Done",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(areAllJobsTerminal(jobs, ["idx:0", "idx:1"])).toBe(true);
    });

    it("returns true when jobs have mixed terminal states", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "completed",
            percentage: 100,
            message: "Done",
            lastUpdate: Date.now(),
          },
        ],
        [
          "idx:1",
          {
            inputIndex: 1,
            label: "file2",
            status: "failed",
            percentage: 50,
            message: "Error",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(areAllJobsTerminal(jobs, ["idx:0", "idx:1"])).toBe(true);
    });

    it("returns false when any job is still processing", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "completed",
            percentage: 100,
            message: "Done",
            lastUpdate: Date.now(),
          },
        ],
        [
          "idx:1",
          {
            inputIndex: 1,
            label: "file2",
            status: "processing",
            percentage: 50,
            message: "Converting",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(areAllJobsTerminal(jobs, ["idx:0", "idx:1"])).toBe(false);
    });
  });

  describe("hasFailedJobs", () => {
    it("returns true when any job failed", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "failed",
            percentage: 50,
            message: "Error",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(hasFailedJobs(jobs)).toBe(true);
    });

    it("returns false when no jobs failed", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "completed",
            percentage: 100,
            message: "Done",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(hasFailedJobs(jobs)).toBe(false);
    });

    it("returns false for empty jobs", () => {
      expect(hasFailedJobs(new Map())).toBe(false);
    });
  });

  describe("hasCancelledJobs", () => {
    it("returns true when any job cancelled", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "cancelled",
            percentage: 30,
            message: "Cancelled",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(hasCancelledJobs(jobs)).toBe(true);
    });

    it("returns false when no jobs cancelled", () => {
      const jobs = new Map<string, JobProgress>([
        [
          "idx:0",
          {
            inputIndex: 0,
            label: "file1",
            status: "completed",
            percentage: 100,
            message: "Done",
            lastUpdate: Date.now(),
          },
        ],
      ]);

      expect(hasCancelledJobs(jobs)).toBe(false);
    });

    it("returns false for empty jobs", () => {
      expect(hasCancelledJobs(new Map())).toBe(false);
    });
  });
});
