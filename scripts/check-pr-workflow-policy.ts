/**
 * Fails when a workflow drifts from the CI policy. Pull-request workflows must
 * keep the trigger types, a stable per-PR concurrency key, and a draft guard on
 * every check job. Every job needs a timeout. Linux runners stay on Ubuntu
 * 24.04. A job that diffs the pull request base fetches that commit or
 * checks out full history. Each error names the fix.
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

interface Step {
	uses?: string;
	run?: string;
	with?: Record<string, unknown>;
	env?: Record<string, unknown>;
}

interface Job {
	if?: string;
	uses?: string;
	'runs-on'?: unknown;
	'timeout-minutes'?: unknown;
	strategy?: { matrix?: unknown };
	steps?: Step[];
}

const REQUIRED_TYPES = [
	'opened',
	'synchronize',
	'reopened',
	'ready_for_review',
	'converted_to_draft',
];
const PR_TRIGGERS = ['pull_request', 'pull_request_target'];
const DRAFT_GUARD = /github\.event\.pull_request\.draft\s*!=\s*true/;
const BASE_SHA = /github\.event\.pull_request\.base\.sha/;
const CLOSED_USE = /github\.event\.action\s*[!=]=\s*'closed'|pull_request\.merged\b/;
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
	if (typeof on === 'string' || Array.isArray(on)) return shorthandTriggers(on);
	if (!on || typeof on !== 'object') return null;
	return on as Record<string, unknown>;
}

function shorthandTriggers(on: string | unknown[]): Record<string, unknown> | null {
	const names = (Array.isArray(on) ? on : [on]).filter((name) => typeof name === 'string');
	const record: Record<string, unknown> = {};
	for (const name of names) {
		if (name === 'pull_request' || name === 'pull_request_target' || name === 'merge_group') {
			record[name] = null;
		}
	}
	return Object.keys(record).length > 0 ? record : null;
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
	for (const name of PR_TRIGGERS) {
		if (name in triggers) findings.push(...prTriggerFindings(file, text, name, triggers[name]));
	}
	return findings;
}

function prTriggerFindings(file: string, text: string, name: string, raw: unknown): Finding[] {
	const config =
		raw && typeof raw === 'object' && !Array.isArray(raw) ? (raw as Record<string, unknown>) : {};
	const types = Array.isArray(config.types) ? config.types.map(String) : [];
	const findings: Finding[] = [];
	const missing = REQUIRED_TYPES.filter((type) => !types.includes(type));
	if (missing.length > 0) {
		findings.push(
			at(
				file,
				text,
				`${name}:`,
				`${name} types are missing ${missing.join(', ')}. Fix: add them and keep the types already listed.`,
			),
		);
	}
	if (CLOSED_USE.test(text) && !types.includes('closed')) {
		findings.push(
			at(
				file,
				text,
				`${name}:`,
				`${name} types are missing closed, and this workflow reads that event. Fix: add closed to the type list.`,
			),
		);
	}
	if (config.paths || config['paths-ignore']) {
		findings.push({
			file,
			line: lineAfter(text, `${name}:`, 'paths'),
			message: `a ${name} path filter blocks the replacement run that cancels in-flight jobs. Fix: remove the filter and select lanes inside the workflow.`,
		});
	}
	return findings;
}

function checksPullRequests(triggers: Record<string, unknown> | null): boolean {
	return Boolean(triggers && PR_TRIGGERS.some((name) => name in triggers));
}

function concurrencyFindings(
	file: string,
	text: string,
	triggers: Record<string, unknown> | null,
	concurrency: unknown,
): Finding[] {
	if (!checksPullRequests(triggers)) return [];
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
	const checksPr = checksPullRequests(triggers);
	const findings: Finding[] = [];
	for (const [name, job] of Object.entries(jobs)) {
		const line = jobLine(text, name);
		findings.push(
			...runnerFindings(file, line, name, job),
			...timeoutFinding(file, line, name, job),
			...baseCheckoutFindings(file, text, name, job),
		);
		if (checksPr && !closedOnly(job) && !draftGuarded(job.if ?? '')) {
			findings.push({
				file,
				line,
				message: draftGuardMessage(name, job.if ?? ''),
			});
		}
	}
	return findings;
}

function runnerFindings(file: string, line: number, name: string, job: Job): Finding[] {
	const labels = runnerLabels(job);
	if (!labels.includes('ubuntu-latest')) return [];
	return [
		{
			file,
			line,
			message: `job ${name} uses ubuntu-latest, which moves to Ubuntu 26 on 2026-10-19. Fix: pin runs-on to ubuntu-24.04.`,
		},
	];
}

function runnerLabels(job: Job): string[] {
	return [...directRunnerLabels(job['runs-on']), ...matrixRunnerLabels(job)];
}

function directRunnerLabels(runsOn: unknown): string[] {
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

function matrixRunnerLabels(job: Job): string[] {
	const runsOn = job['runs-on'];
	if (typeof runsOn !== 'string') return [];
	const key = runsOn.match(/matrix\.([A-Za-z_][\w]*)/)?.[1];
	const matrix = job.strategy?.matrix;
	if (!key || !matrix || typeof matrix !== 'object' || Array.isArray(matrix)) return [];
	const record = matrix as Record<string, unknown>;
	const labels = stringValues(record[key]);
	const include = record.include;
	if (!Array.isArray(include)) return labels;
	for (const row of include) {
		if (row && typeof row === 'object' && key in row) {
			const value = (row as Record<string, unknown>)[key];
			if (typeof value === 'string') labels.push(value);
		}
	}
	return labels;
}

function stringValues(value: unknown): string[] {
	if (!Array.isArray(value)) return [];
	return value.filter((item): item is string => typeof item === 'string');
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

function draftGuarded(iff: string): boolean {
	return DRAFT_GUARD.test(iff) && !hasTopLevelOr(expressionBody(iff));
}

function expressionBody(iff: string): string {
	return iff.replace(/^\s*\$\{\{\s*/, '').replace(/\s*\}\}\s*$/, '');
}

function hasTopLevelOr(expression: string): boolean {
	let depth = 0;
	let quote = '';
	for (let index = 0; index < expression.length; index++) {
		const char = expression[index];
		if (quote) {
			if (char === '\\') index += 1;
			else if (char === quote) quote = '';
			continue;
		}
		if (char === "'" || char === '"') {
			quote = char;
			continue;
		}
		if (char === '(') depth += 1;
		else if (char === ')') depth = Math.max(0, depth - 1);
		else if (depth === 0 && expression.startsWith('||', index)) return true;
	}
	return false;
}

function draftGuardMessage(name: string, iff: string): string {
	if (DRAFT_GUARD.test(iff) && hasTopLevelOr(expressionBody(iff))) {
		return `job ${name} bypasses its draft guard with a top-level ||. Fix: combine \`github.event.pull_request.draft != true\` with &&, and keep any || inside parentheses.`;
	}
	return `job ${name} runs on a draft pull request. Fix: add \`github.event.pull_request.draft != true\` to its if, including reusable-workflow callers and aggregate jobs. A step guard still allocates a runner.`;
}

function closedOnly(job: Job): boolean {
	const iff = job.if ?? '';
	return /github\.event\.action\s*==\s*'closed'/.test(iff) && !draftGuarded(iff);
}

function baseCheckoutFindings(file: string, text: string, name: string, job: Job): Finding[] {
	const steps = stepsOf(job);
	if (!diffsPullRequestBase(steps) || fetchesBaseOrFullHistory(steps)) return [];
	return [
		{
			file,
			line: lineAfter(text, `  ${name}:`, 'pull_request.base.sha'),
			message: `job ${name} diffs the pull request base from a shallow checkout that does not contain that commit. Fix: \`git fetch --no-tags --depth=1 origin\` the base sha, or set checkout fetch-depth to 0.`,
		},
	];
}

function stepsOf(job: Job): Step[] {
	if (!Array.isArray(job.steps)) return [];
	return job.steps.filter((step) => step !== null && typeof step === 'object');
}

function diffsPullRequestBase(steps: Step[]): boolean {
	const blob = steps.map((step) => `${step.run ?? ''}\n${envText(step)}`).join('\n');
	if (!BASE_SHA.test(blob)) return false;
	return steps.some((step) => runDiffsBase(step.run ?? '', envText(step), blob));
}

function runDiffsBase(run: string, stepEnv: string, jobBlob: string): boolean {
	if (!/\bverification\.ts\s+plan\b|\bgit\s+diff\b/.test(run)) return false;
	if (BASE_SHA.test(run) || BASE_SHA.test(stepEnv)) return true;
	return /\$\{?\s*BASE\b/.test(run) && /BASE=.*pull_request\.base\.sha/.test(jobBlob);
}

function fetchesBaseOrFullHistory(steps: Step[]): boolean {
	const envBlob = steps.map((step) => envText(step)).join('\n');
	return steps.some((step) => fullHistory(step) || fetchesBaseSha(step.run ?? '', envBlob));
}

function fullHistory(step: Step): boolean {
	if (!step.uses?.startsWith('actions/checkout')) return false;
	const depth = checkoutDepth(step);
	return depth === 0 || depth === '0';
}

function checkoutDepth(step: Step): unknown {
	const inputs = step.with;
	if (!inputs || typeof inputs !== 'object' || Array.isArray(inputs)) return undefined;
	return inputs['fetch-depth'];
}

function fetchesBaseSha(run: string, envBlob: string): boolean {
	if (!/\bgit\s+fetch\b/.test(run)) return false;
	if (BASE_SHA.test(run) || /--unshallow\b/.test(run)) return true;
	return /\$\{?\s*BASE\b/.test(run) && /BASE=.*pull_request\.base\.sha/.test(envBlob);
}

function envText(step: Step): string {
	if (!step.env || typeof step.env !== 'object' || Array.isArray(step.env)) return '';
	return Object.entries(step.env)
		.map(([key, value]) => `${key}=${String(value)}`)
		.join('\n');
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
