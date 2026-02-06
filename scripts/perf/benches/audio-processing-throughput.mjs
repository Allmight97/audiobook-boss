import { spawnSync } from "node:child_process";
import { statSync } from "node:fs";
import { resolve } from "node:path";

const SYNTHETIC_WORKLOAD = {
  seconds: 22,
  sampleRate: 44_100,
  channels: 2,
  passes: 4,
};

const REAL_WORKLOAD = {
  fixturePath: "media/media_30sec.mp3",
  fixtureDurationSeconds: 30,
};

function runSyntheticThroughput() {
  const frameCount = SYNTHETIC_WORKLOAD.seconds * SYNTHETIC_WORKLOAD.sampleRate;
  const totalSamples = frameCount * SYNTHETIC_WORKLOAD.channels;
  const samples = new Float32Array(totalSamples);

  for (let i = 0; i < samples.length; i += 1) {
    samples[i] = Math.sin(i * 0.001) * 0.5;
  }

  const start = performance.now();
  let checksum = 0;
  for (let pass = 0; pass < SYNTHETIC_WORKLOAD.passes; pass += 1) {
    const gain = 0.7 + pass * 0.03;
    for (let i = 0; i < samples.length; i += 1) {
      const value = Math.max(-1, Math.min(1, samples[i] * gain));
      checksum += value;
    }
  }
  const elapsedMs = performance.now() - start;

  const bytesProcessed =
    samples.byteLength * SYNTHETIC_WORKLOAD.passes;
  const mibPerSecond =
    elapsedMs > 0 ? bytesProcessed / (1024 * 1024) / (elapsedMs / 1000) : 0;

  return {
    value: mibPerSecond,
    details: {
      mode: "synthetic",
      bytes_processed: bytesProcessed,
      elapsed_ms: Number(elapsedMs.toFixed(3)),
      throughput_mib_per_s: Number(mibPerSecond.toFixed(3)),
      checksum: Number(checksum.toFixed(3)),
      frames: frameCount,
      channels: SYNTHETIC_WORKLOAD.channels,
      passes: SYNTHETIC_WORKLOAD.passes,
    },
  };
}

function runFfprobeDuration(filePath) {
  const probe = spawnSync(
    "ffprobe",
    [
      "-v",
      "error",
      "-show_entries",
      "format=duration",
      "-of",
      "default=nokey=1:noprint_wrappers=1",
      filePath,
    ],
    {
      encoding: "utf8",
    }
  );

  if (probe.status !== 0) {
    return null;
  }

  const parsed = Number.parseFloat(probe.stdout.trim());
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }
  return parsed;
}

function runRealThroughput(repoRoot) {
  const fixture = resolve(repoRoot, REAL_WORKLOAD.fixturePath);
  const duration = runFfprobeDuration(fixture) ?? REAL_WORKLOAD.fixtureDurationSeconds;
  const fixtureBytes = statSync(fixture).size;

  const start = performance.now();
  const proc = spawnSync(
    "ffmpeg",
    [
      "-v",
      "error",
      "-nostats",
      "-i",
      fixture,
      "-f",
      "null",
      "-",
    ],
    {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }
  );
  const elapsedMs = performance.now() - start;

  if (proc.status !== 0) {
    throw new Error(`ffmpeg failed: ${proc.stderr.trim() || "unknown error"}`);
  }

  const realtimeFactor = elapsedMs > 0 ? duration / (elapsedMs / 1000) : 0;
  const throughputMibPerSecond =
    elapsedMs > 0 ? fixtureBytes / (1024 * 1024) / (elapsedMs / 1000) : 0;

  return {
    value: throughputMibPerSecond,
    details: {
      mode: "real",
      fixture,
      fixture_bytes: fixtureBytes,
      fixture_duration_seconds: Number(duration.toFixed(3)),
      elapsed_ms: Number(elapsedMs.toFixed(3)),
      throughput_mib_per_s: Number(throughputMibPerSecond.toFixed(3)),
      realtime_factor: Number(realtimeFactor.toFixed(3)),
    },
  };
}

export const benchmark = {
  name: "audio-processing-throughput",
  description: "Audio throughput benchmark using synthetic DSP and real FFmpeg decode path.",
  phase: 1,
  metricType: "throughput_mib_per_s",
  direction: "higher_is_better",
  warmupRuns: 1,
  async run({ mode, repoRoot }) {
    if (mode === "real") {
      return runRealThroughput(repoRoot);
    }

    return runSyntheticThroughput();
  },
};
