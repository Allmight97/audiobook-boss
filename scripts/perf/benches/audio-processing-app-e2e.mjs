import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const REAL_WORKLOAD = {
	preferredFixturePath: 'media/Feedback.m4b',
	defaultClipSeconds: 300,
};

function getEncoderScenarios() {
	const bitrateKbps = resolvePositiveInt(process.env.ABB_PERF_AAC_BITRATE_KBPS, 64);
	const nativeTwoloop = process.env.ABB_PERF_NATIVE_TWOOLOOP !== '0';
	const fdkVbr = Math.min(5, Math.max(1, resolvePositiveInt(process.env.ABB_PERF_FDK_VBR, 3)));
	const fdkAfterburner = process.env.ABB_PERF_FDK_AFTERBURNER !== '0';

	return [
		{
			key: 'native_aac',
			availableKey: 'aac',
			args: ['--bitrate-kbps', `${bitrateKbps}`, '--native-twoloop', nativeTwoloop ? '1' : '0'],
		},
		{
			key: 'aac_at',
			availableKey: 'aac_at',
			args: ['--bitrate-kbps', `${bitrateKbps}`],
		},
		{
			key: 'fdk_he_aac',
			availableKey: 'libfdk_aac',
			args: [
				'--bitrate-kbps',
				`${bitrateKbps}`,
				'--fdk-vbr',
				`${fdkVbr}`,
				'--fdk-afterburner',
				fdkAfterburner ? '1' : '0',
			],
		},
	];
}

function detectAvailableEncoders() {
	const proc = spawnSync('ffmpeg', ['-hide_banner', '-encoders'], {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
	});

	const output = `${proc.stdout ?? ''}\n${proc.stderr ?? ''}`;
	return {
		aac: /\baac\b/.test(output),
		aac_at: /\baac_at\b/.test(output),
		libfdk_aac: /\blibfdk_aac\b/.test(output),
	};
}

function resolvePositiveInt(raw, fallback) {
	const parsed = Number(raw);
	if (!Number.isFinite(parsed) || parsed <= 0) {
		return fallback;
	}
	return Math.floor(parsed);
}

function runFfprobeDuration(filePath) {
	const probe = spawnSync(
		'ffprobe',
		[
			'-v',
			'error',
			'-show_entries',
			'format=duration',
			'-of',
			'default=nokey=1:noprint_wrappers=1',
			filePath,
		],
		{
			encoding: 'utf8',
		},
	);

	if (probe.status !== 0) {
		throw new Error(`ffprobe failed for '${filePath}': ${probe.stderr.trim() || 'unknown error'}`);
	}

	const parsed = Number.parseFloat(probe.stdout.trim());
	if (!Number.isFinite(parsed) || parsed <= 0) {
		throw new Error(
			`ffprobe returned invalid duration for '${filePath}': '${probe.stdout.trim()}'`,
		);
	}
	return parsed;
}

function resolveClipSeconds(fixtureDuration) {
	const envClip = Number(process.env.ABB_PERF_AUDIO_MAX_SECONDS);
	if (Number.isFinite(envClip) && envClip > 0) {
		return envClip;
	}
	return Math.min(fixtureDuration, REAL_WORKLOAD.defaultClipSeconds);
}

function ensureAppE2eBinary(repoRoot) {
	const build = spawnSync(
		'cargo',
		['build', '--bin', 'perf_app_e2e', '--manifest-path', 'src-tauri/Cargo.toml'],
		{
			cwd: repoRoot,
			encoding: 'utf8',
			stdio: ['ignore', 'pipe', 'pipe'],
		},
	);
	if (build.status !== 0) {
		throw new Error(`Failed to build perf_app_e2e binary:\n${build.stderr || build.stdout}`);
	}
	const exe = process.platform === 'win32' ? 'perf_app_e2e.exe' : 'perf_app_e2e';
	const binaryPath = resolve(repoRoot, 'target', 'debug', exe);
	if (!existsSync(binaryPath)) {
		throw new Error(`perf_app_e2e binary not found at '${binaryPath}' after build`);
	}
	return binaryPath;
}

function parseLastJsonLine(stdout) {
	const lines = `${stdout ?? ''}`
		.trim()
		.split(/\r?\n/)
		.map((line) => line.trim())
		.filter(Boolean);
	for (let i = lines.length - 1; i >= 0; i -= 1) {
		try {
			return JSON.parse(lines[i]);
		} catch {
			// continue
		}
	}
	throw new Error('Failed to parse JSON output from perf_app_e2e');
}

function runEncoderScenario({ binaryPath, fixture, clipSeconds, scenario }) {
	const tempDir = mkdtempSync(join(tmpdir(), 'abb-app-e2e-perf-'));
	const outputPath = join(tempDir, `app-${scenario.key}.m4b`);

	try {
		const proc = spawnSync(
			binaryPath,
			[
				'--input',
				fixture,
				'--output',
				outputPath,
				'--encoder',
				scenario.key,
				'--preview-seconds',
				String(clipSeconds),
				...scenario.args,
			],
			{
				encoding: 'utf8',
				stdio: ['ignore', 'pipe', 'pipe'],
			},
		);

		if (proc.status !== 0) {
			throw new Error(
				`${scenario.key}: ${proc.stderr?.trim() || proc.stdout?.trim() || 'perf_app_e2e failed'}`,
			);
		}

		const payload = parseLastJsonLine(proc.stdout);
		const outputPreviewPath =
			typeof payload.output_preview === 'string' ? payload.output_preview : null;
		const outputForSize =
			outputPreviewPath && existsSync(outputPreviewPath) ? outputPreviewPath : outputPath;
		const outputBytes = existsSync(outputForSize) ? statSync(outputForSize).size : 0;
		const elapsedSeconds = Number(payload.elapsed_ms) / 1000;
		const throughputMibPerSecond =
			elapsedSeconds > 0 ? outputBytes / (1024 * 1024) / elapsedSeconds : 0;

		return {
			encoder: scenario.key,
			elapsed_ms: Number(payload.elapsed_ms),
			output_bytes: outputBytes,
			processed_seconds: Number(payload.processed_seconds),
			realtime_factor: Number(payload.realtime_factor),
			throughput_mib_per_s: Number(throughputMibPerSecond.toFixed(3)),
			resolved_encoder: payload.resolved_encoder ?? scenario.key,
		};
	} finally {
		rmSync(tempDir, { recursive: true, force: true });
	}
}

function runRealAppE2e(repoRoot) {
	const preferredFixture = resolve(repoRoot, REAL_WORKLOAD.preferredFixturePath);
	const envFixture = process.env.ABB_PERF_AUDIO_INPUT;
	const fixture = envFixture ? resolve(repoRoot, envFixture) : preferredFixture;

	if (!existsSync(fixture)) {
		throw new Error(
			`Real-mode fixture not found at '${fixture}'. Add ${REAL_WORKLOAD.preferredFixturePath} or set ABB_PERF_AUDIO_INPUT.`,
		);
	}

	const availableEncoders = detectAvailableEncoders();
	const fixtureDuration = runFfprobeDuration(fixture);
	const clipSeconds = resolveClipSeconds(fixtureDuration);
	const binaryPath = ensureAppE2eBinary(repoRoot);
	const scenarios = getEncoderScenarios();
	const runs = [];
	const skipped = [];

	for (const scenario of scenarios) {
		if (!availableEncoders[scenario.availableKey]) {
			skipped.push({
				encoder: scenario.key,
				reason: `${scenario.availableKey} not available in ffmpeg build`,
			});
			continue;
		}
		runs.push(
			runEncoderScenario({
				binaryPath,
				fixture,
				clipSeconds,
				scenario,
			}),
		);
	}

	if (runs.length === 0) {
		throw new Error('No configured AAC encoders are available for app_e2e real-mode run.');
	}

	const avgRealtimeFactor = runs.reduce((sum, run) => sum + run.realtime_factor, 0) / runs.length;
	const avgThroughputMibPerSecond =
		runs.reduce((sum, run) => sum + run.throughput_mib_per_s, 0) / runs.length;

	return {
		value: avgRealtimeFactor,
		details: {
			mode: 'real',
			layer: 'app_e2e',
			fixture,
			fixture_duration_seconds: Number(fixtureDuration.toFixed(3)),
			clip_seconds: Number(clipSeconds.toFixed(3)),
			average_realtime_factor: Number(avgRealtimeFactor.toFixed(3)),
			average_throughput_mib_per_s: Number(avgThroughputMibPerSecond.toFixed(3)),
			encoder_runs: runs,
			skipped_encoders: skipped,
		},
	};
}

export const benchmark = {
	name: 'audio-processing-app-e2e',
	description:
		'Measures full app encode pipeline throughput (decode/resample/encode/finalize) per encoder path.',
	userImpact:
		'Shows app end-to-end encoding speed so regressions can be attributed beyond encoder-only benchmarks',
	phase: 1,
	metricType: 'realtime_factor',
	direction: 'higher_is_better',
	warmupRuns: 1,
	async run({ mode, repoRoot }) {
		if (mode !== 'real') {
			return {
				skipped: true,
				reason: 'audio-processing-app-e2e is real-mode only',
			};
		}
		return runRealAppE2e(repoRoot);
	},
};
