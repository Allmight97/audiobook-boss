import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import {
	buildReport,
	buildRunReport,
	classifyRun,
	type Fetcher,
	isLanesStep,
	type JobInfo,
	parseRustLog,
	renderReport,
	splitJobTiming,
} from './ci-timing';

const fixtureDir = fileURLToPath(new URL('./fixtures/ci-timing/', import.meta.url));

const postMergeRun = 41360600256;
const exactRun = 41099036623;
const prRun = 40453687407;
const scheduleRun = 40882844258;

interface RunFixture {
	run: { id: number; event: string; display_title: string; created_at: string };
	jobs: JobInfo[];
}

function runFixture(runId: number): RunFixture {
	return JSON.parse(readFileSync(`${fixtureDir}run-${runId}.json`, 'utf8')) as RunFixture;
}

function logFixture(name: string): string {
	return readFileSync(`${fixtureDir}${name}.txt`, 'utf8');
}

const logByJobId: Record<number, string> = {
	125010013564: logFixture('rust-macos-miss-refused'),
	125010013598: logFixture('rust-linux-exact'),
	124212388563: logFixture('rust-macos-exact'),
	124212388460: logFixture('rust-linux-exact'),
	122022060734: logFixture('rust-linux-fallback'),
	122022060742: logFixture('rust-macos-miss'),
	123478789048: logFixture('rust-linux-miss'),
	123478789100: logFixture('rust-macos-miss'),
};

function fixtureFetcher(fetchedLogs: number[] = []): Fetcher {
	return {
		listRunIds: () => [postMergeRun],
		run: (runId) => runFixture(runId).run,
		jobs: (runId) => runFixture(runId).jobs,
		jobLog: (jobId) => {
			fetchedLogs.push(jobId);
			return logByJobId[jobId] ?? '';
		},
	};
}

function jobNamed(runId: number, name: string): JobInfo {
	const job = runFixture(runId).jobs.find((candidate) => candidate.name === name);
	if (!job) throw new Error(`fixture run ${runId} has no job '${name}'`);
	return job;
}

describe('classifyRun', () => {
	it('calls a pull_request run with a non-skipped cache cleanup job closed', () => {
		const { run, jobs } = runFixture(postMergeRun);
		expect(classifyRun(run.event, jobs)).toBe('closed');
	});

	it('calls an ordinary pull_request run pr, even though its cleanup job is skipped', () => {
		const { run, jobs } = runFixture(prRun);
		expect(jobs.some((job) => job.name === 'Delete closed PR caches')).toBe(true);
		expect(classifyRun(run.event, jobs)).toBe('pr');
	});

	it('calls a schedule run that fired seven hours after its cron schedule-full when Rust ran', () => {
		const { run, jobs } = runFixture(scheduleRun);
		expect(run.created_at).toBe('2026-10-29T13:32:21Z');
		expect(classifyRun(run.event, jobs)).toBe('schedule-full');
	});

	it('calls a schedule run without Rust jobs schedule-advisory', () => {
		const { run, jobs } = runFixture(scheduleRun);
		const withoutRust = jobs.filter((job) => !job.name.startsWith('Rust'));
		expect(classifyRun(run.event, withoutRust)).toBe('schedule-advisory');
	});

	it('maps push and workflow_dispatch and rejects other events', () => {
		expect(classifyRun('push', [])).toBe('push');
		expect(classifyRun('workflow_dispatch', [])).toBe('dispatch');
		expect(() => classifyRun('issues', [])).toThrow("Unsupported workflow event 'issues'");
	});
});

describe('splitJobTiming', () => {
	it('splits a Rust job into queue, setup, lanes, and post that add up to the total', () => {
		const timing = splitJobTiming(jobNamed(postMergeRun, 'Rust (macos-latest)'));
		expect(timing).toEqual({
			queueSeconds: 3,
			setupSeconds: 140,
			lanesSeconds: 726,
			postSeconds: 28,
			totalSeconds: 894,
		});
		expect(
			(timing.setupSeconds ?? 0) + (timing.lanesSeconds ?? 0) + (timing.postSeconds ?? 0),
		).toBe(timing.totalSeconds);
	});

	it('finds the lanes step by either name used in CI', () => {
		expect(isLanesStep('Run bash scripts/verify.sh core')).toBe(true);
		expect(isLanesStep('Run read -r -a lanes <<< "$LANES"')).toBe(true);
		expect(isLanesStep('Run bash scripts/setup.sh rust')).toBe(false);
	});

	it('reports only queue and total for a job without a lanes step', () => {
		const timing = splitJobTiming(jobNamed(postMergeRun, 'changes'));
		expect(timing).toEqual({
			queueSeconds: 3,
			setupSeconds: null,
			lanesSeconds: null,
			postSeconds: null,
			totalSeconds: 13,
		});
	});

	it('reads the lanes seconds from the Core job that runs verify.sh directly', () => {
		const timing = splitJobTiming(jobNamed(postMergeRun, 'Core crate tests and clippy'));
		expect(timing.lanesSeconds).not.toBeNull();
		expect(timing.totalSeconds).toBe(36);
	});
});

describe('parseRustLog', () => {
	it('reads a miss, the cache key, the Rust version, and a refused save', () => {
		const report = parseRustLog(logFixture('rust-macos-miss-refused'));
		expect(report).toMatchObject({
			rustCache: 'miss',
			cacheKey: 'v0-rust-rust-Darwin-arm64-b1372a82-bbb68f93',
			rustVersions: ['1.99.1 aarch64-apple-darwin 58cf8f9c35b355199727627d07ae34a4e2846d67'],
			saveRefused: true,
			ffmpegCache: 'hit',
		});
	});

	it('reads an exact hit with no refused save', () => {
		const report = parseRustLog(logFixture('rust-macos-exact'));
		expect(report).toMatchObject({ rustCache: 'exact', saveRefused: false, ffmpegCache: 'hit' });
		expect(report.cacheKey).toMatch(/^v0-rust-rust-Darwin-arm64-/);
	});

	it('reads a fallback restore and every Rust version the cache key considered', () => {
		const report = parseRustLog(logFixture('rust-linux-fallback'));
		expect(report.rustCache).toBe('fallback');
		expect(report.rustVersions).toEqual([
			'1.99.0 x86_64-unknown-linux-gnu b940084d7eb6a299eb4bfeb8e34901bc051e7ac4',
			'1.99.1 x86_64-unknown-linux-gnu 58cf8f9c35b355199727627d07ae34a4e2846d67',
		]);
	});

	it('reports unknown cache results and an ffmpeg miss for a log without them', () => {
		const report = parseRustLog('2026-11-01T00:00:00.0000000Z Set up job\n');
		expect(report).toEqual({
			rustCache: 'unknown',
			cacheKey: null,
			rustVersions: [],
			saveRefused: false,
			ffmpegCache: 'miss',
			lanes: [],
		});
	});

	it('strips the byte-order mark and terminal escapes before matching', () => {
		const log = logFixture('rust-macos-miss-refused');
		expect(log.startsWith('﻿')).toBe(true);
		expect(log).toContain('\u001b[36;1m');
		const withEscapes = log.replace('No cache found.', '\u001b[31mNo cache found.\u001b[0m');
		expect(parseRustLog(withEscapes).rustCache).toBe('miss');
	});
});

describe('lane timing', () => {
	it('times each lane from its header to its own PASS line and ignores the closing summary', () => {
		const { lanes } = parseRustLog(logFixture('rust-macos-miss-refused'));
		expect(lanes).toEqual([
			{ lane: 'core', status: 'PASS', seconds: 25 },
			{ lane: 'engine', status: 'PASS', seconds: 293 },
			{ lane: 'media', status: 'PASS', seconds: 64 },
			{ lane: 'host', status: 'PASS', seconds: 324 },
			{ lane: 'decrypt', status: 'PASS', seconds: 14 },
			{ lane: 'apple', status: 'PASS', seconds: 6 },
		]);
	});

	it('records FAIL and SKIP outcomes and marks a lane that never ended', () => {
		const log = [
			'2026-11-01T00:00:00.0000000Z ======== core ========',
			'2026-11-01T00:00:10.0000000Z FAIL core',
			'2026-11-01T00:00:10.5000000Z ======== apple ========',
			'2026-11-01T00:00:10.9000000Z SKIP apple: macOS only',
			'2026-11-01T00:00:11.0000000Z SKIP apple',
			'2026-11-01T00:00:12.0000000Z ======== host ========',
		].join('\n');
		expect(parseRustLog(log).lanes).toEqual([
			{ lane: 'core', status: 'FAIL', seconds: 10 },
			{ lane: 'apple', status: 'SKIP', seconds: 0 },
			{ lane: 'host', status: 'unfinished', seconds: null },
		]);
	});
});

describe('buildReport', () => {
	const runIds = [postMergeRun, exactRun, prRun, scheduleRun];

	it('reports each run kind and Rust cache outcome', () => {
		const { runs } = buildReport(fixtureFetcher(), runIds);
		const outcomes = runs.map((run) => ({
			id: run.id,
			kind: run.kind,
			rust: Object.fromEntries(
				run.jobs.filter((job) => job.rust).map((job) => [job.name, job.rust?.rustCache]),
			),
		}));
		expect(outcomes).toEqual([
			{
				id: postMergeRun,
				kind: 'closed',
				rust: { 'Rust (macos-latest)': 'miss', 'Rust (ubuntu-24.04)': 'exact' },
			},
			{
				id: exactRun,
				kind: 'closed',
				rust: { 'Rust (macos-latest)': 'exact', 'Rust (ubuntu-24.04)': 'exact' },
			},
			{
				id: prRun,
				kind: 'pr',
				rust: { 'Rust (macos-latest)': 'miss', 'Rust (ubuntu-24.04)': 'fallback' },
			},
			{
				id: scheduleRun,
				kind: 'schedule-full',
				rust: { 'Rust (macos-latest)': 'miss', 'Rust (ubuntu-24.04)': 'miss' },
			},
		]);
	});

	it('fetches logs only for Rust jobs that ran', () => {
		const fetchedLogs: number[] = [];
		const report = buildRunReport(fixtureFetcher(fetchedLogs), scheduleRun);
		expect(fetchedLogs.sort()).toEqual([123478789048, 123478789100]);
		expect(report.jobs.map((job) => job.name)).not.toContain('Delete closed PR caches');
	});

	it('groups Rust job totals by run kind and OS', () => {
		const { summary } = buildReport(fixtureFetcher(), runIds);
		const closedMac = summary.find((row) => row.kind === 'closed' && row.os === 'macos-latest');
		expect(closedMac).toMatchObject({ jobs: 2, exact: 1, fallback: 0, miss: 1 });
		expect(closedMac?.maxSeconds).toBe(894);
		expect(closedMac?.minSeconds).toBeLessThan(894);
		expect(closedMac?.medianSeconds).toBe(Math.round(((closedMac?.minSeconds ?? 0) + 894) / 2));
		const prLinux = summary.find((row) => row.kind === 'pr' && row.os === 'ubuntu-24.04');
		expect(prLinux).toMatchObject({ jobs: 1, fallback: 1 });
		expect(summary.map((row) => `${row.kind}/${row.os}`)).toEqual([
			'closed/macos-latest',
			'closed/ubuntu-24.04',
			'pr/macos-latest',
			'pr/ubuntu-24.04',
			'schedule-full/macos-latest',
			'schedule-full/ubuntu-24.04',
		]);
	});

	it('renders a table row per job with cache result, key, and a refused save', () => {
		const text = renderReport(buildReport(fixtureFetcher(), [postMergeRun]));
		expect(text).toContain('## Run 41360600256: closed');
		expect(text).toContain(
			'| Rust (macos-latest) | 3s | 2m20s | 12m06s | 28s | 14m54s | rust miss, ffmpeg hit, save refused | `v0-rust-rust-Darwin-arm64-b1372a82-bbb68f93` |',
		);
		expect(text).toContain('- Rust (macos-latest) lanes: core 25s, engine 4m53s');
		expect(text).toContain('## Summary: Rust job totals by run kind and OS');
	});
});
