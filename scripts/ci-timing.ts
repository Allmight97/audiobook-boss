/**
 * Where CI time goes, and whether the caches hit.
 *
 * Usage: bun scripts/ci-timing.ts [--limit N] [--run ID ...] [--json]
 *   --limit N   last N completed, non-cancelled runs of ci.yml (default 20, max 100)
 *   --run ID    report these run ids instead (repeatable)
 *   --json      print JSON instead of markdown
 *
 * Reads GitHub Actions through `gh api` for the current repository. A
 * `closed` run is a pull request's closed event; before the push-to-main
 * trigger it also re-ran the suite after a merge.
 */
import { execFileSync } from 'node:child_process';
import { parseArgs } from 'node:util';

export type RunKind = 'pr' | 'closed' | 'push' | 'schedule-full' | 'schedule-advisory' | 'dispatch';

export type RustCacheOutcome = 'exact' | 'fallback' | 'miss' | 'unknown';

export interface RunInfo {
	id: number;
	event: string;
	display_title: string;
	created_at: string;
}

export interface StepInfo {
	name: string;
	conclusion: string | null;
	started_at: string | null;
	completed_at: string | null;
}

export interface JobInfo {
	id: number;
	name: string;
	conclusion: string | null;
	created_at: string;
	started_at: string | null;
	completed_at: string | null;
	steps: StepInfo[];
}

export interface JobTiming {
	queueSeconds: number;
	setupSeconds: number | null;
	lanesSeconds: number | null;
	postSeconds: number | null;
	totalSeconds: number;
}

export interface LaneTiming {
	lane: string;
	status: 'PASS' | 'FAIL' | 'SKIP' | 'unfinished';
	seconds: number | null;
}

export interface RustLogReport {
	rustCache: RustCacheOutcome;
	cacheKey: string | null;
	rustVersions: string[];
	saveRefused: boolean;
	ffmpegCache: 'hit' | 'miss';
	lanes: LaneTiming[];
}

export interface JobReport extends JobTiming {
	id: number;
	name: string;
	rust: RustLogReport | null;
}

export interface RunReport {
	id: number;
	kind: RunKind;
	event: string;
	title: string;
	createdAt: string;
	jobs: JobReport[];
}

export interface SummaryRow {
	kind: RunKind;
	os: string;
	jobs: number;
	medianSeconds: number;
	minSeconds: number;
	maxSeconds: number;
	exact: number;
	fallback: number;
	miss: number;
}

export interface Report {
	runs: RunReport[];
	summary: SummaryRow[];
}

export interface Fetcher {
	listRunIds(limit: number): number[];
	run(runId: number): RunInfo;
	jobs(runId: number): JobInfo[];
	jobLog(jobId: number): string;
}

const rustJobPattern = /^Rust \((.+)\)$/;
const deleteCachesJobName = 'Delete closed PR caches';

function seconds(from: string, to: string): number {
	const millis = (iso: string) => Date.parse(iso.replace(/(\.\d{3})\d*Z$/, '$1Z'));
	return Math.round((millis(to) - millis(from)) / 1000);
}

export function ranJob(job: JobInfo): boolean {
	return job.conclusion !== 'skipped' && job.started_at !== null && job.completed_at !== null;
}

export function classifyRun(event: string, jobs: JobInfo[]): RunKind {
	const ran = jobs.filter(ranJob);
	switch (event) {
		case 'pull_request':
			return ran.some((job) => job.name === deleteCachesJobName) ? 'closed' : 'pr';
		case 'push':
			return 'push';
		case 'schedule':
			return ran.some((job) => rustJobPattern.test(job.name))
				? 'schedule-full'
				: 'schedule-advisory';
		case 'workflow_dispatch':
			return 'dispatch';
		default:
			throw new Error(`Unsupported workflow event '${event}'.`);
	}
}

export function isLanesStep(name: string): boolean {
	return name.startsWith('Run bash scripts/verify.sh') || name.includes('read -r -a lanes');
}

export function splitJobTiming(job: JobInfo): JobTiming {
	if (job.started_at === null || job.completed_at === null) {
		throw new Error(`Job '${job.name}' has no start or end time.`);
	}
	const lanesStep = job.steps.find(
		(step) =>
			isLanesStep(step.name) &&
			step.conclusion !== 'skipped' &&
			step.started_at !== null &&
			step.completed_at !== null,
	);
	const timing = {
		queueSeconds: seconds(job.created_at, job.started_at),
		totalSeconds: seconds(job.started_at, job.completed_at),
	};
	if (!lanesStep?.started_at || !lanesStep.completed_at) {
		return { ...timing, setupSeconds: null, lanesSeconds: null, postSeconds: null };
	}
	return {
		...timing,
		setupSeconds: seconds(job.started_at, lanesStep.started_at),
		lanesSeconds: seconds(lanesStep.started_at, lanesStep.completed_at),
		postSeconds: seconds(lanesStep.completed_at, job.completed_at),
	};
}

interface LogLine {
	timestamp: string;
	text: string;
}

function stripEscapes(text: string): string {
	return text
		.replaceAll('﻿', '')
		.split('\u001b')
		.map((part, index) => (index === 0 ? part : part.replace(/^\[[0-9;?]*[ -/]*[@-~]/, '')))
		.join('');
}

function parseLogLines(log: string): LogLine[] {
	return stripEscapes(log)
		.split('\n')
		.flatMap((line) => {
			const match = /^(\d{4}-\d\d-\d\dT[\d:.]+Z) ?(.*?)\r?$/.exec(line);
			return match?.[1] !== undefined ? [{ timestamp: match[1], text: match[2] ?? '' }] : [];
		});
}

function rustCacheOutcome(lines: LogLine[]): RustCacheOutcome {
	for (const { text } of lines) {
		if (text.includes('full match: true')) return 'exact';
		if (text.includes('full match: false')) return 'fallback';
		if (text.includes('No cache found.')) return 'miss';
	}
	return 'unknown';
}

function lineAfter(lines: LogLine[], heading: string): string | null {
	const index = lines.findIndex(({ text }) => text.trim() === heading);
	return index < 0 ? null : (lines[index + 1]?.text.trim() ?? null);
}

function listedRustVersions(lines: LogLine[]): string[] {
	const start = lines.findIndex(({ text }) => text.trim() === '- Rust Versions:');
	if (start < 0) return [];
	const versions: string[] = [];
	for (const { text } of lines.slice(start + 1)) {
		const entry = /^ {4}- (.+)$/.exec(text);
		if (!entry?.[1]) break;
		versions.push(entry[1]);
	}
	return versions;
}

function laneTimings(lines: LogLine[]): LaneTiming[] {
	const lanes: LaneTiming[] = [];
	let open: { lane: string; startedAt: string } | null = null;
	for (const { timestamp, text } of lines) {
		const start = /^======== (\S+) ========$/.exec(text);
		if (start?.[1]) {
			if (open) lanes.push({ lane: open.lane, status: 'unfinished', seconds: null });
			open = { lane: start[1], startedAt: timestamp };
			continue;
		}
		const end = /^(PASS|FAIL|SKIP) ([\w-]+)/.exec(text);
		if (open && end?.[2] === open.lane) {
			lanes.push({
				lane: open.lane,
				status: end[1] as 'PASS' | 'FAIL' | 'SKIP',
				seconds: seconds(open.startedAt, timestamp),
			});
			open = null;
		}
	}
	if (open) lanes.push({ lane: open.lane, status: 'unfinished', seconds: null });
	return lanes;
}

export function parseRustLog(log: string): RustLogReport {
	const lines = parseLogLines(log);
	return {
		rustCache: rustCacheOutcome(lines),
		cacheKey: lineAfter(lines, 'Cache Key:'),
		rustVersions: listedRustVersions(lines),
		saveRefused: lines.some(({ text }) => text.includes('does not permit writes')),
		ffmpegCache: lines.some(({ text }) => text.startsWith('Cache hit for: ffmpeg-portable-'))
			? 'hit'
			: 'miss',
		lanes: laneTimings(lines),
	};
}

function median(values: number[]): number {
	const sorted = [...values].toSorted((a, b) => a - b);
	const middle = Math.floor(sorted.length / 2);
	const upper = sorted[middle] ?? 0;
	const lower = sorted[middle - 1] ?? upper;
	return sorted.length % 2 === 1 ? upper : Math.round((lower + upper) / 2);
}

export function summarize(runs: RunReport[]): SummaryRow[] {
	const groups = new Map<string, { kind: RunKind; os: string; jobs: JobReport[] }>();
	for (const run of runs) {
		for (const job of run.jobs) {
			const os = rustJobPattern.exec(job.name)?.[1];
			if (!os || !job.rust) continue;
			const group = groups.get(`${run.kind}\t${os}`) ?? { kind: run.kind, os, jobs: [] };
			group.jobs.push(job);
			groups.set(`${run.kind}\t${os}`, group);
		}
	}
	return [...groups.values()]
		.map(({ kind, os, jobs }) => {
			const totals = jobs.map((job) => job.totalSeconds);
			const count = (outcome: RustCacheOutcome) =>
				jobs.filter((job) => job.rust?.rustCache === outcome).length;
			return {
				kind,
				os,
				jobs: jobs.length,
				medianSeconds: median(totals),
				minSeconds: Math.min(...totals),
				maxSeconds: Math.max(...totals),
				exact: count('exact'),
				fallback: count('fallback'),
				miss: count('miss'),
			};
		})
		.toSorted((a, b) => a.kind.localeCompare(b.kind) || a.os.localeCompare(b.os));
}

export function buildRunReport(fetcher: Fetcher, runId: number): RunReport {
	const run = fetcher.run(runId);
	const allJobs = fetcher.jobs(runId);
	const jobs = allJobs.filter(ranJob).map((job) => ({
		id: job.id,
		name: job.name,
		...splitJobTiming(job),
		rust: rustJobPattern.test(job.name) ? parseRustLog(fetcher.jobLog(job.id)) : null,
	}));
	return {
		id: run.id,
		kind: classifyRun(run.event, allJobs),
		event: run.event,
		title: run.display_title,
		createdAt: run.created_at,
		jobs,
	};
}

export function buildReport(fetcher: Fetcher, runIds: number[]): Report {
	const runs = runIds.map((runId) => buildRunReport(fetcher, runId));
	return { runs, summary: summarize(runs) };
}

function duration(value: number | null): string {
	if (value === null) return '-';
	if (value < 60) return `${value}s`;
	return `${Math.floor(value / 60)}m${String(value % 60).padStart(2, '0')}s`;
}

function cacheCell(rust: RustLogReport | null): string {
	if (!rust) return '-';
	const parts = [`rust ${rust.rustCache}`, `ffmpeg ${rust.ffmpegCache}`];
	if (rust.saveRefused) parts.push('save refused');
	return parts.join(', ');
}

function jobRow(job: JobReport): string {
	const cells = [
		job.name,
		duration(job.queueSeconds),
		duration(job.setupSeconds),
		duration(job.lanesSeconds),
		duration(job.postSeconds),
		duration(job.totalSeconds),
		cacheCell(job.rust),
		job.rust?.cacheKey ? `\`${job.rust.cacheKey}\`` : '-',
	];
	return `| ${cells.join(' | ')} |`;
}

function rustDetail(job: JobReport): string[] {
	if (!job.rust) return [];
	const lanes = job.rust.lanes.map(
		(lane) =>
			`${lane.lane} ${lane.status === 'PASS' ? '' : `${lane.status} `}${duration(lane.seconds)}`,
	);
	return [
		`- ${job.name} lanes: ${lanes.join(', ') || 'none'}`,
		`- ${job.name} rust: ${job.rust.rustVersions.join('; ') || 'unknown'}`,
	];
}

export function renderRun(run: RunReport): string {
	return [
		`## Run ${run.id}: ${run.kind} (${run.event}, ${run.createdAt}) ${run.title}`,
		'',
		'| job | queue | setup | lanes | post | total | cache | key |',
		'| --- | --- | --- | --- | --- | --- | --- | --- |',
		...run.jobs.map(jobRow),
		'',
		...run.jobs.flatMap(rustDetail),
	].join('\n');
}

export function renderSummary(summary: SummaryRow[]): string {
	return [
		'## Summary: Rust job totals by run kind and OS',
		'',
		'| kind | os | jobs | median | range | exact | fallback | miss |',
		'| --- | --- | --- | --- | --- | --- | --- | --- |',
		...summary.map(
			(row) =>
				`| ${row.kind} | ${row.os} | ${row.jobs} | ${duration(row.medianSeconds)} | ${duration(row.minSeconds)} to ${duration(row.maxSeconds)} | ${row.exact} | ${row.fallback} | ${row.miss} |`,
		),
	].join('\n');
}

export function renderReport(report: Report): string {
	return [...report.runs.map(renderRun), renderSummary(report.summary)].join('\n\n');
}

function gh(args: string[]): string {
	return execFileSync('gh', ['api', ...args], {
		encoding: 'utf8',
		maxBuffer: 256 * 1024 * 1024,
		stdio: ['ignore', 'pipe', 'inherit'],
	});
}

function ghJson<T>(path: string): T {
	return JSON.parse(gh([path])) as T;
}

export const ghFetcher: Fetcher = {
	listRunIds(limit) {
		const { workflow_runs } = ghJson<{
			workflow_runs: { id: number; conclusion: string | null }[];
		}>('repos/{owner}/{repo}/actions/workflows/ci.yml/runs?status=completed&per_page=100');
		return workflow_runs
			.filter((run) => run.conclusion !== 'cancelled')
			.slice(0, limit)
			.map((run) => run.id);
	},
	run: (runId) => ghJson<RunInfo>(`repos/{owner}/{repo}/actions/runs/${runId}`),
	jobs: (runId) =>
		ghJson<{ jobs: JobInfo[] }>(`repos/{owner}/{repo}/actions/runs/${runId}/jobs?per_page=100`)
			.jobs,
	jobLog: (jobId) =>
		gh(['--allow-escape-sequences', `repos/{owner}/{repo}/actions/jobs/${jobId}/logs`]),
};

function parseLimit(value: string | undefined): number {
	const limit = value === undefined ? 20 : Number(value);
	if (!Number.isInteger(limit) || limit < 1 || limit > 100) {
		throw new Error(`--limit must be an integer from 1 to 100, got '${value}'.`);
	}
	return limit;
}

function main(): void {
	const { values } = parseArgs({
		args: process.argv.slice(2),
		options: {
			limit: { type: 'string' },
			run: { type: 'string', multiple: true },
			json: { type: 'boolean' },
		},
	});
	const runIds = values.run?.length
		? values.run.map((id) => {
				if (!/^\d+$/.test(id)) throw new Error(`--run needs a numeric run id, got '${id}'.`);
				return Number(id);
			})
		: ghFetcher.listRunIds(parseLimit(values.limit));
	const report = buildReport(ghFetcher, runIds);
	console.log(values.json ? JSON.stringify(report, null, 2) : renderReport(report));
}

if (import.meta.main) {
	try {
		main();
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
