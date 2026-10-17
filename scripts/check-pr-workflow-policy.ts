/**
 * Fails when a workflow drifts from the CI policy. Pull-request workflows must
 * keep the trigger types, a stable per-PR concurrency key, and a draft guard on
 * every check job. Every job needs a timeout. Linux runners stay on Ubuntu
 * 24.04. Each error names the fix.
 *
 * Usage: bun scripts/check-pr-workflow-policy.ts [workflow.yml ...]
 * With no paths, checks every workflow under .github/workflows.
 */
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { parseDocument } from 'yaml';

export interface Finding {
	file: string;
	line: number;
	message: string;
}

interface Job {
	if?: string;
	uses?: string;
	'runs-on'?: unknown;
	'timeout-minutes'?: unknown;
}

const REQUIRED_TYPES = [
	'opened',
	'synchronize',
	'reopened',
	'ready_for_review',
	'converted_to_draft',
];
const DRAFT_GUARD = /github\.event\.pull_request\.draft\s*!=\s*true/;
const UNSTABLE_KEY = /github\.(?:sha|run_id|run_attempt)\b|head\.sha/;

export function checkWorkflow(file: string, text: string): Finding[] {
	const parsed = parseDocument(text);
	if (parsed.errors.length > 0) {
		return parsed.errors.map((error) => ({
			file,
			line: error.linePos?.[0].line ?? 1,
			message: `workflow YAML did not parse (${error.message}). Fix: correct the syntax.`,
		}));
	}
	const workflow = parsed.toJS() as Record<string, unknown>;
	const triggers = triggerRecord(workflow);
	const jobs = jobRecord(workflow.jobs);
	return [
		...triggerFindings(file, text, triggers),
		...concurrencyFindings(file, text, triggers, workflow.concurrency),
		...jobFindings(file, text, triggers, jobs),
	];
}

function triggerRecord(workflow: Record<string, unknown>): Record<string, unknown> | null {
	const on = workflow.on ?? workflow.true;
	if (!on || typeof on !== 'object' || Array.isArray(on)) return null;
	return on as Record<string, unknown>;
}

function jobRecord(jobs: unknown): Record<string, Job> {
	if (!jobs || typeof jobs !== 'object' || Array.isArray(jobs)) return {};
	return jobs as Record<string, Job>;
}

function triggerFindings(
	file: string,
	text: string,
	triggers: Record<string, unknown> | null,
): Finding[] {
	if (!triggers) return [];
	const findings: Finding[] = [];
	if ('merge_group' in triggers) {
		findings.push(
			at(
				file,
				text,
				'merge_group:',
				'merge_group never runs: this repository is user-owned, so merge queues do not apply. Fix: remove the merge_group trigger.',
			),
		);
	}
	if (!('pull_request' in triggers)) return findings;
	const pullRequest = triggers.pull_request;
	const config =
		pullRequest && typeof pullRequest === 'object' && !Array.isArray(pullRequest)
			? (pullRequest as Record<string, unknown>)
			: {};
	const types = Array.isArray(config.types) ? config.types.map(String) : [];
	const missing = REQUIRED_TYPES.filter((type) => !types.includes(type));
	if (missing.length > 0) {
		findings.push(
			at(
				file,
				text,
				'pull_request:',
				`pull_request types are missing ${missing.join(', ')}. Fix: add them and keep the types already listed, including closed.`,
			),
		);
	}
	if (config.paths || config['paths-ignore']) {
		findings.push({
			file,
			line: lineAfter(text, 'pull_request:', 'paths'),
			message:
				'a pull_request path filter blocks the replacement run that cancels in-flight jobs. Fix: remove the filter and select lanes inside the workflow.',
		});
	}
	return findings;
}

function concurrencyFindings(
	file: string,
	text: string,
	triggers: Record<string, unknown> | null,
	concurrency: unknown,
): Finding[] {
	if (!triggers || !('pull_request' in triggers)) return [];
	const group = concurrencyGroup(concurrency);
	const problems: string[] = [];
	if (!group.includes('github.workflow')) {
		problems.push('include github.workflow so caller keys stay distinct');
	}
	if (!group.includes('github.event.pull_request.number')) {
		problems.push('include github.event.pull_request.number');
	}
	if (!group.includes('github.ref')) {
		problems.push('keep github.ref as the fallback when there is no pull request');
	}
	if (UNSTABLE_KEY.test(group)) {
		problems.push('remove the sha or run id so a new push cancels the old run');
	}
	if (!cancelsInProgress(concurrency)) {
		problems.push('set cancel-in-progress: true');
	}
	if (triggers.workflow_call && !group.includes("'")) {
		problems.push('add a literal prefix; a reusable workflow inherits the caller github.workflow');
	}
	if (problems.length === 0) return [];
	return [
		at(
			file,
			text,
			'concurrency:',
			`concurrency key is not the stable per-PR policy (${problems.join('; ')}). Fix: group: \${{ github.workflow }}-\${{ github.event.pull_request.number || github.ref }} and cancel-in-progress: true.`,
		),
	];
}

function concurrencyGroup(concurrency: unknown): string {
	if (typeof concurrency === 'string') return concurrency;
	if (!concurrency || typeof concurrency !== 'object') return '';
	const group = (concurrency as { group?: unknown }).group;
	return typeof group === 'string' ? group : '';
}

function cancelsInProgress(concurrency: unknown): boolean {
	if (!concurrency || typeof concurrency !== 'object') return false;
	return (concurrency as { 'cancel-in-progress'?: unknown })['cancel-in-progress'] === true;
}

function jobFindings(
	file: string,
	text: string,
	triggers: Record<string, unknown> | null,
	jobs: Record<string, Job>,
): Finding[] {
	const checksPullRequests = Boolean(triggers && 'pull_request' in triggers);
	const findings: Finding[] = [];
	for (const [name, job] of Object.entries(jobs)) {
		const line = jobLine(text, name);
		findings.push(
			...runnerFindings(file, line, name, job),
			...timeoutFinding(file, line, name, job),
		);
		if (checksPullRequests && !closedOnly(job) && !DRAFT_GUARD.test(job.if ?? '')) {
			findings.push({
				file,
				line,
				message: `job ${name} runs on a draft pull request. Fix: add \`github.event.pull_request.draft != true\` to its if, including reusable-workflow callers and aggregate jobs. A step guard still allocates a runner.`,
			});
		}
	}
	return findings;
}

function runnerFindings(file: string, line: number, name: string, job: Job): Finding[] {
	const labels = runnerLabels(job['runs-on']);
	if (!labels.includes('ubuntu-latest')) return [];
	return [
		{
			file,
			line,
			message: `job ${name} uses ubuntu-latest, which moves to Ubuntu 26 on 2026-10-19. Fix: pin runs-on to ubuntu-24.04.`,
		},
	];
}

function runnerLabels(runsOn: unknown): string[] {
	if (typeof runsOn === 'string') return [runsOn];
	if (Array.isArray(runsOn)) return runsOn.map(String);
	if (
		runsOn &&
		typeof runsOn === 'object' &&
		Array.isArray((runsOn as { labels?: unknown }).labels)
	) {
		return ((runsOn as { labels: unknown[] }).labels ?? []).map(String);
	}
	return [];
}

function timeoutFinding(file: string, line: number, name: string, job: Job): Finding[] {
	const timeout = job['timeout-minutes'];
	if (typeof timeout === 'number' && timeout > 0) return [];
	return [
		{
			file,
			line,
			message: `job ${name} has no timeout-minutes. Fix: set timeout-minutes so a hung job cannot hold a runner.`,
		},
	];
}

function closedOnly(job: Job): boolean {
	const iff = job.if ?? '';
	return /github\.event\.action\s*==\s*'closed'/.test(iff) && !DRAFT_GUARD.test(iff);
}

function jobLine(text: string, name: string): number {
	const index = text.split('\n').indexOf(`  ${name}:`);
	return index === -1 ? 1 : index + 1;
}

function at(file: string, text: string, needle: string, message: string): Finding {
	return { file, line: lineAfter(text, needle, needle), message };
}

function lineAfter(text: string, start: string, needle: string): number {
	const lines = text.split('\n');
	const startAt = lines.findIndex((line) => line.includes(start));
	if (startAt < 0) return 1;
	const next = lines.slice(startAt).findIndex((line) => line.includes(needle));
	return next < 0 ? startAt + 1 : startAt + next + 1;
}

function workflowPaths(repoRoot: string): string[] {
	const directory = path.join(repoRoot, '.github/workflows');
	return readdirSync(directory)
		.filter((name) => name.endsWith('.yml') || name.endsWith('.yaml'))
		.map((name) => path.join('.github/workflows', name))
		.sort();
}

function main(): number {
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const args = process.argv.slice(2);
	const files = args.length > 0 ? args : workflowPaths(repoRoot);
	const findings = files.flatMap((file) =>
		checkWorkflow(file, readFileSync(path.resolve(repoRoot, file), 'utf8')),
	);
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
