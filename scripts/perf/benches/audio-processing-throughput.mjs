import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const SYNTHETIC_WORKLOAD = {
  seconds: 22,
  sampleRate: 44_100,
  channels: 2,
  passes: 4,
};

const REAL_WORKLOAD = {
  preferredFixturePath: "media/Feedback.m4b",
  defaultClipSeconds: 300,
};

function getEncoderScenarios() {
  const bitrateKbps = resolvePositiveInt(process.env.ABB_PERF_AAC_BITRATE_KBPS, 64);
  const nativeTwoloop = process.env.ABB_PERF_NATIVE_TWOOLOOP !== "0";
  const fdkVbr = Math.min(5, Math.max(1, resolvePositiveInt(process.env.ABB_PERF_FDK_VBR, 3)));
  const fdkAfterburner = process.env.ABB_PERF_FDK_AFTERBURNER !== "0";

  return [
    {
      key: "native_aac",
      ffmpegName: "aac",
      args: [
        "-c:a",
        "aac",
        "-b:a",
        `${bitrateKbps}k`,
        ...(nativeTwoloop ? ["-aac_coder", "twoloop"] : []),
      ],
    },
    {
      key: "aac_at",
      ffmpegName: "aac_at",
      args: ["-c:a", "aac_at", "-b:a", `${bitrateKbps}k`, "-aac_at_mode", "cvbr"],
    },
    {
      key: "fdk_he_aac",
      ffmpegName: "libfdk_aac",
      args: [
        "-c:a",
        "libfdk_aac",
        "-profile:a",
        "aac_he",
        "-vbr",
        `${fdkVbr}`,
        "-afterburner",
        fdkAfterburner ? "1" : "0",
      ],
    },
  ];
}

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
      layer: "app_pipeline_synthetic",
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
    throw new Error(`ffprobe failed for '${filePath}': ${probe.stderr.trim() || "unknown error"}`);
  }

  const parsed = Number.parseFloat(probe.stdout.trim());
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`ffprobe returned invalid duration for '${filePath}': '${probe.stdout.trim()}'`);
  }
  return parsed;
}

function runRealThroughput(repoRoot) {
  const preferredFixture = resolve(repoRoot, REAL_WORKLOAD.preferredFixturePath);
  const envFixture = process.env.ABB_PERF_AUDIO_INPUT;
  const fixture = envFixture ? resolve(repoRoot, envFixture) : preferredFixture;

  if (!existsSync(fixture)) {
    throw new Error(
      `Real-mode fixture not found at '${fixture}'. Add ${REAL_WORKLOAD.preferredFixturePath} or set ABB_PERF_AUDIO_INPUT.`
    );
  }

  const availableEncoders = detectAvailableEncoders();
  const fixtureDuration = runFfprobeDuration(fixture);
  const clipSeconds = resolveClipSeconds(fixtureDuration);
  const processedSeconds = Math.min(fixtureDuration, clipSeconds);
  const fixtureBytes = statSync(fixture).size;

  const runs = [];
  const skipped = [];
  const scenarios = getEncoderScenarios();

  for (const scenario of scenarios) {
    if (!availableEncoders[scenario.ffmpegName]) {
      skipped.push({
        encoder: scenario.key,
        reason: `${scenario.ffmpegName} not available in ffmpeg build`,
      });
      continue;
    }

    const output = runEncoderScenario({
      fixture,
      clipSeconds,
      processedSeconds,
      scenario,
    });
    runs.push(output);
  }

  if (runs.length === 0) {
    throw new Error("No configured AAC encoders are available for real-mode run.");
  }

  const avgRealtimeFactor =
    runs.reduce((sum, run) => sum + run.realtime_factor, 0) / runs.length;
  const avgThroughputMibPerSecond =
    runs.reduce((sum, run) => sum + run.throughput_mib_per_s, 0) / runs.length;

  return {
    value: avgRealtimeFactor,
    details: {
      mode: "real",
      layer: "encoder_cli",
      fixture,
      fixture_bytes: fixtureBytes,
      fixture_duration_seconds: Number(fixtureDuration.toFixed(3)),
      clip_seconds: Number(clipSeconds.toFixed(3)),
      processed_seconds: Number(processedSeconds.toFixed(3)),
      available_encoders: availableEncoders,
      scenario_config: {
        bitrate_kbps: resolvePositiveInt(process.env.ABB_PERF_AAC_BITRATE_KBPS, 64),
        native_twoloop: process.env.ABB_PERF_NATIVE_TWOOLOOP !== "0",
        fdk_vbr: Math.min(5, Math.max(1, resolvePositiveInt(process.env.ABB_PERF_FDK_VBR, 3))),
        fdk_afterburner: process.env.ABB_PERF_FDK_AFTERBURNER !== "0",
      },
      average_realtime_factor: Number(avgRealtimeFactor.toFixed(3)),
      average_throughput_mib_per_s: Number(avgThroughputMibPerSecond.toFixed(3)),
      encoder_runs: runs,
      skipped_encoders: skipped,
    },
  };
}

function detectAvailableEncoders() {
  const proc = spawnSync("ffmpeg", ["-hide_banner", "-encoders"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });

  const output = `${proc.stdout ?? ""}\n${proc.stderr ?? ""}`;
  return {
    aac: /\baac\b/.test(output),
    aac_at: /\baac_at\b/.test(output),
    libfdk_aac: /\blibfdk_aac\b/.test(output),
  };
}

function resolveClipSeconds(fixtureDuration) {
  const envClip = Number(process.env.ABB_PERF_AUDIO_MAX_SECONDS);
  if (Number.isFinite(envClip) && envClip > 0) {
    return envClip;
  }
  return Math.min(fixtureDuration, REAL_WORKLOAD.defaultClipSeconds);
}

function resolvePositiveInt(raw, fallback) {
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return fallback;
  }
  return Math.floor(parsed);
}

function runEncoderScenario({ fixture, clipSeconds, processedSeconds, scenario }) {
  const tempDir = mkdtempSync(join(tmpdir(), "abb-perf-"));
  const outFile = join(tempDir, `encoded-${scenario.key}.m4b`);

  try {
    const start = performance.now();
    const proc = spawnSync(
      "ffmpeg",
      [
        "-v",
        "error",
        "-nostats",
        "-y",
        "-i",
        fixture,
        "-map",
        "0:a:0",
        "-t",
        clipSeconds.toString(),
        ...scenario.args,
        outFile,
      ],
      {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      }
    );
    const elapsedMs = performance.now() - start;

    if (proc.status !== 0) {
      throw new Error(`${scenario.key}: ${proc.stderr.trim() || "ffmpeg failed"}`);
    }

    const outputBytes = statSync(outFile).size;
    const elapsedSeconds = elapsedMs / 1000;
    const realtimeFactor =
      elapsedSeconds > 0 ? processedSeconds / elapsedSeconds : 0;
    const throughputMibPerSecond =
      elapsedSeconds > 0 ? outputBytes / (1024 * 1024) / elapsedSeconds : 0;

    return {
      encoder: scenario.key,
      ffmpeg_name: scenario.ffmpegName,
      elapsed_ms: Number(elapsedMs.toFixed(3)),
      output_bytes: outputBytes,
      realtime_factor: Number(realtimeFactor.toFixed(3)),
      throughput_mib_per_s: Number(throughputMibPerSecond.toFixed(3)),
    };
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

export const benchmark = {
  name: "audio-processing-throughput",
  description:
    "Audio throughput benchmark using synthetic DSP and real encoder-path transcodes.",
  userImpact: "Audiobooks encode fast — a 33-min book should finish in seconds, not minutes",
  phase: 1,
  metricType: "realtime_factor",
  direction: "higher_is_better",
  warmupRuns: 1,
  async run({ mode, repoRoot }) {
    if (mode === "real") {
      return runRealThroughput(repoRoot);
    }

    return runSyntheticThroughput();
  },
};
