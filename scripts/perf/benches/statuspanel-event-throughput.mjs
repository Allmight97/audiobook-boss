import { STAGES } from "../../../src/types/events.ts";
import { calculateAggregateProgress, deriveAggregateStage } from "../../../src/ui/statusPanel/domain/aggregate.ts";
import { buildJobKey } from "../../../src/ui/statusPanel/domain/jobKeys.ts";
import { buildQueueSnapshotState } from "../../../src/ui/statusPanel/domain/queueState.ts";
import { formatAggregateMessage } from "../../../src/ui/statusPanel/formatting.ts";
import {
  isTerminalProgressEvent,
  shouldThrottleProgressUpdate,
} from "../../../src/ui/statusPanel/services/progressThrottle.ts";

const WORKLOADS = {
  synthetic: {
    jobs: 240,
    updatesPerJob: 40,
    tickMs: 260,
  },
  real: {
    jobs: 500,
    updatesPerJob: 48,
    tickMs: 210,
  },
};

function buildQueueItems(jobCount) {
  return Array.from({ length: jobCount }, (_, index) => ({
    input_index: index,
    file_path: `/library/Author ${index % 25}/Book ${index}.mp3`,
  }));
}

function stageForPercent(percentage) {
  if (percentage >= 100) return STAGES.completed;
  if (percentage >= 85) return STAGES.writing;
  if (percentage >= 25) return STAGES.converting;
  return STAGES.analyzing;
}

function generateProgressEvents(workload) {
  const events = [];
  const increment = Math.max(1, Math.floor(100 / workload.updatesPerJob));

  for (let job = 0; job < workload.jobs; job += 1) {
    const jobId = `job-${job}`;
    for (let percentage = increment; percentage <= 100; percentage += increment) {
      const bounded = Math.min(100, percentage);
      const stage = stageForPercent(bounded);
      events.push({
        stage,
        percentage: bounded,
        message: `Processing ${jobId} (${bounded}%)`,
        current_file: `Book ${job} (${job + 1}/${workload.jobs})`,
        job_id: jobId,
        input_index: job,
      });
    }
  }

  return events;
}

export const benchmark = {
  name: "statuspanel-event-throughput",
  description: "StatusPanel event handling throughput using domain/service helpers.",
  userImpact: "UI handles rapid progress updates without stuttering during batch jobs",
  phase: 1,
  metricType: "events_per_second",
  direction: "higher_is_better",
  warmupRuns: 1,
  async run({ mode }) {
    const workload = WORKLOADS[mode] ?? WORKLOADS.synthetic;
    const queueItems = buildQueueItems(workload.jobs);
    const events = generateProgressEvents(workload);
    const seedTime = Date.now();
    const snapshot = buildQueueSnapshotState(queueItems, seedTime);

    const jobProgress = new Map(snapshot.jobProgress);
    const queueOrder = [...snapshot.queueOrder];
    const lastRenderByKey = new Map();

    let accepted = 0;
    let throttled = 0;

    let eventNow = seedTime;
    const start = performance.now();

    for (const event of events) {
      const jobKey = buildJobKey(event.input_index, event.job_id);
      const terminal = isTerminalProgressEvent(event);
      const lastRender = lastRenderByKey.get(jobKey) ?? 0;

      if (shouldThrottleProgressUpdate(eventNow, lastRender, terminal)) {
        throttled += 1;
        eventNow += workload.tickMs;
        continue;
      }

      lastRenderByKey.set(jobKey, eventNow);
      const existing = jobProgress.get(jobKey);
      const status = terminal ? event.stage : "processing";

      jobProgress.set(jobKey, {
        jobId: event.job_id ?? existing?.jobId,
        inputIndex:
          typeof event.input_index === "number"
            ? event.input_index
            : existing?.inputIndex,
        label: existing?.label ?? event.current_file ?? `Job ${jobKey}`,
        status,
        stage: event.stage,
        percentage: Math.round(event.percentage * 10) / 10,
        message: event.message,
        lastUpdate: eventNow,
      });

      if (typeof event.input_index === "number") {
        const queueKey = buildJobKey(event.input_index, undefined);
        if (!queueOrder.includes(queueKey)) {
          queueOrder.push(queueKey);
        }
      }

      const aggregate = calculateAggregateProgress(jobProgress);
      deriveAggregateStage(jobProgress);
      formatAggregateMessage(jobProgress, aggregate);

      accepted += 1;
      eventNow += workload.tickMs;
    }

    const elapsedMs = performance.now() - start;
    if (elapsedMs <= 0) {
      throw new Error("statuspanel-event-throughput elapsed time was <= 0ms");
    }

    const throughput = accepted / (elapsedMs / 1000);

    return {
      value: throughput,
      details: {
        jobs: workload.jobs,
        updates_per_job: workload.updatesPerJob,
        input_events: events.length,
        accepted_events: accepted,
        throttled_events: throttled,
        elapsed_ms: Number(elapsedMs.toFixed(3)),
        ms_per_event: Number((elapsedMs / accepted).toFixed(4)),
        aggregate: calculateAggregateProgress(jobProgress),
      },
    };
  },
};
