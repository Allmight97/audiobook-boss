import { readFileSync } from 'node:fs';

export type SessionHealth = 'clean' | 'degraded' | 'failed' | 'interrupted' | 'indeterminate';

export interface OperationOutcome {
	id: string;
	kind: string;
	status: string;
	total: number;
	succeeded: number;
	skipped: number;
	cancelled: number;
	failed: number;
	terminal: boolean;
}

export interface JobOutcome {
	id: string;
	operationId: string;
	inputIndex: string;
	kind: string;
	status: string;
	elapsedMs?: number;
	code?: string;
	category?: string;
	terminal: boolean;
}

export interface DevLogAnalysis {
	health: SessionHealth;
	reasons: string[];
	operations: OperationOutcome[];
	jobs: JobOutcome[];
	unmatchedOperationIds: string[];
	unmatchedJobIds: string[];
	wrapperExitStatus: number;
	appStarts: number;
	appRestarts: number;
	viteRestarts: number;
	viteInternalErrors: number;
	rustErrors: number;
	rustWarnings: number;
	actionableRustWarnings: number;
	panics: number;
	compilerFailures: number;
	malformedLifecycleLines: number;
	orphanOperationIds: string[];
	orphanJobIds: string[];
	childExitCodes: number[];
	encoderLines: number;
	externalFdkRuns: number;
	externalFdkStatuses: Record<string, number>;
	malformedExternalFdkRuns: number;
	highSignalLines: string[];
}

interface MutableOperation extends OperationOutcome {
	sawStart: boolean;
	sawTerminal: boolean;
}

interface MutableJob extends JobOutcome {
	sawStart: boolean;
	sawTerminal: boolean;
}

interface ExternalFdkRun {
	status?: string;
	jobId?: string;
}

const ESCAPE = String.fromCharCode(27);
const BELL = String.fromCharCode(7);
const ANSI_PATTERN = new RegExp(
	`${ESCAPE}(?:\\[[0-?]*[ -/]*[@-~]|\\][^${ESCAPE}${BELL}]*(?:${BELL}|${ESCAPE}\\\\))`,
	'g',
);
const EXPECTED_SIGNAL_EXIT_CODES = new Set([130, 143]);
const OPERATION_EVENT_STATUSES: Record<string, ReadonlySet<string>> = {
	accepted: new Set(['accepted']),
	running: new Set(['running']),
	cancel_requested: new Set(['cancelling']),
	terminal: new Set(['completed', 'cancelled', 'failed', 'mixed']),
};
const JOB_EVENT_STATUSES: Record<string, ReadonlySet<string>> = {
	started: new Set(['running']),
	terminal: new Set(['success', 'cancelled', 'failed']),
};
const EXTERNAL_FDK_STATUSES = new Set(['success', 'failed', 'wait_error', 'interrupted']);

export function stripAnsi(value: string): string {
	return value.replace(ANSI_PATTERN, '').replaceAll('\r', '');
}

function countMatches(input: string, pattern: RegExp): number {
	return input.match(pattern)?.length ?? 0;
}

function nonNegativeInteger(value: string | undefined): number | null {
	if (!value || !/^\d+$/.test(value)) return null;
	const parsed = Number.parseInt(value, 10);
	return Number.isSafeInteger(parsed) ? parsed : null;
}

function parseRecord(line: string, recordName: string): Record<string, string> | null {
	const marker = `${recordName} `;
	const markerIndex = line.indexOf(marker);
	if (markerIndex < 0) return null;

	const fields: Record<string, string> = {};
	for (const token of line
		.slice(markerIndex + marker.length)
		.trim()
		.split(/\s+/)) {
		const equalsIndex = token.indexOf('=');
		if (equalsIndex <= 0 || equalsIndex === token.length - 1) continue;
		fields[token.slice(0, equalsIndex)] = token.slice(equalsIndex + 1);
	}
	return fields;
}

function operationFromFields(fields: Record<string, string>): MutableOperation | null {
	const id = fields.operation_id;
	const event = fields.event;
	const kind = fields.kind;
	const status = fields.status;
	const allowedStatuses = event ? OPERATION_EVENT_STATUSES[event] : undefined;
	const total = nonNegativeInteger(fields.total);
	const succeeded = nonNegativeInteger(fields.succeeded);
	const skipped = nonNegativeInteger(fields.skipped);
	const cancelled = nonNegativeInteger(fields.cancelled);
	const failed = nonNegativeInteger(fields.failed);
	if (
		!id ||
		!kind ||
		!status ||
		!allowedStatuses?.has(status) ||
		total === null ||
		succeeded === null ||
		skipped === null ||
		cancelled === null ||
		failed === null
	) {
		return null;
	}

	return {
		id,
		kind,
		status,
		total,
		succeeded,
		skipped,
		cancelled,
		failed,
		terminal: event === 'terminal',
		sawStart: event === 'accepted',
		sawTerminal: event === 'terminal',
	};
}

function jobFromFields(fields: Record<string, string>): MutableJob | null {
	const id = fields.job_id;
	const event = fields.event;
	const operationId = fields.operation_id;
	const inputIndex = fields.input_index;
	const kind = fields.kind;
	const status = fields.status;
	const allowedStatuses = event ? JOB_EVENT_STATUSES[event] : undefined;
	const elapsedMs = fields.elapsed_ms ? nonNegativeInteger(fields.elapsed_ms) : undefined;
	const terminal = event === 'terminal';
	const hasCode = Boolean(fields.code);
	const hasCategory = Boolean(fields.category);
	const failureFieldsAreValid =
		status === 'failed' ? hasCode && hasCategory : !hasCode && !hasCategory;
	if (
		!id ||
		!operationId ||
		!inputIndex ||
		!(inputIndex === 'none' || /^\d+$/.test(inputIndex)) ||
		!kind ||
		!status ||
		!allowedStatuses?.has(status) ||
		(terminal && elapsedMs == null) ||
		(!terminal && fields.elapsed_ms !== undefined) ||
		!failureFieldsAreValid
	) {
		return null;
	}
	return {
		id,
		operationId,
		inputIndex,
		kind,
		status,
		elapsedMs,
		code: fields.code,
		category: fields.category,
		terminal,
		sawStart: !terminal,
		sawTerminal: terminal,
	};
}

function mergeOperation(
	operations: Map<string, MutableOperation>,
	next: MutableOperation,
): boolean {
	const current = operations.get(next.id);
	if (current && (current.kind !== next.kind || current.sawTerminal || next.sawStart)) {
		return false;
	}
	operations.set(next.id, {
		...current,
		...next,
		sawStart: Boolean(current?.sawStart || next.sawStart),
		sawTerminal: Boolean(current?.sawTerminal || next.sawTerminal),
		terminal: Boolean(current?.sawTerminal || next.sawTerminal),
	});
	return true;
}

function mergeJob(jobs: Map<string, MutableJob>, next: MutableJob): boolean {
	const current = jobs.get(next.id);
	if (
		current &&
		(current.operationId !== next.operationId ||
			current.inputIndex !== next.inputIndex ||
			current.kind !== next.kind ||
			current.sawTerminal ||
			(current.sawStart && next.sawStart))
	) {
		return false;
	}
	jobs.set(next.id, {
		...current,
		...next,
		sawStart: Boolean(current?.sawStart || next.sawStart),
		sawTerminal: Boolean(current?.sawTerminal || next.sawTerminal),
		terminal: Boolean(current?.sawTerminal || next.sawTerminal),
	});
	return true;
}

function parseLegacyJobLine(
	line: string,
	jobs: Map<string, MutableJob>,
): 'none' | 'valid' | 'malformed' {
	const started = line.match(/\bJob ([0-9a-fA-F-]+) started for output:/);
	if (started) {
		const merged = mergeJob(jobs, {
			id: started[1],
			operationId: 'unknown',
			inputIndex: 'none',
			kind: 'unknown',
			status: 'running',
			terminal: false,
			sawStart: true,
			sawTerminal: false,
		});
		return merged ? 'valid' : 'malformed';
	}

	const terminal = line.match(/\bJob ([0-9a-fA-F-]+) (completed successfully|cancelled:|failed:)/);
	if (!terminal) return 'none';
	const current = jobs.get(terminal[1]);
	if (!current?.sawStart || current.kind !== 'unknown') return 'none';
	const status = terminal[2].startsWith('completed')
		? 'success'
		: terminal[2].startsWith('cancelled')
			? 'cancelled'
			: 'failed';
	const merged = mergeJob(jobs, {
		...current,
		status,
		terminal: true,
		sawStart: false,
		sawTerminal: true,
	});
	return merged ? 'valid' : 'malformed';
}

function sortedValues<T extends { id: string }>(values: Map<string, T>): T[] {
	return [...values.values()].sort((left, right) => left.id.localeCompare(right.id));
}

function parseExternalFdkRuns(input: string): {
	runs: ExternalFdkRun[];
	malformedRuns: number;
} {
	type MutableExternalFdkRun = ExternalFdkRun & { malformed: boolean; inStderr: boolean };
	const runs: ExternalFdkRun[] = [];
	let malformedRuns = 0;
	let current: MutableExternalFdkRun | undefined;

	const finishCurrent = (closed: boolean): void => {
		if (!current) {
			malformedRuns += 1;
			return;
		}
		const { malformed, inStderr: _inStderr, ...run } = current;
		runs.push(run);
		if (!closed || malformed || !run.status || !EXTERNAL_FDK_STATUSES.has(run.status)) {
			malformedRuns += 1;
		}
		current = undefined;
	};

	for (const line of input.split('\n')) {
		if (line.startsWith('--- external-fdk run ')) {
			if (current) finishCurrent(false);
			current = { malformed: false, inStderr: false };
			continue;
		}
		if (line === '--- end external-fdk run ---') {
			finishCurrent(true);
			continue;
		}
		if (!current) continue;
		if (line === 'stderr:') {
			current.inStderr = true;
			continue;
		}
		if (current.inStderr) continue;
		if (line.startsWith('status=')) {
			if (current.status !== undefined) current.malformed = true;
			current.status = line.slice('status='.length);
		} else if (line.startsWith('job_id=')) {
			if (current.jobId !== undefined) current.malformed = true;
			current.jobId = line.slice('job_id='.length);
		}
	}
	if (current) finishCurrent(false);

	return { runs, malformedRuns };
}

function isExpectedCancellationWarning(line: string, hasCancelledJob: boolean): boolean {
	if (/processing_job .*\bstatus=cancelled\b/.test(line)) return true;
	if (/\bJob [0-9a-fA-F-]+ cancelled:/.test(line)) return true;
	return (
		hasCancelledJob &&
		(/Processing was cancelled/i.test(line) ||
			/External ffmpeg stderr before early exit:/i.test(line))
	);
}

const FAILURE_EXPLANATIONS: Record<string, string> = {
	file_validation_failed: 'A file or output-path validation check failed.',
	invalid_input: 'The processing request contained invalid input.',
	io_error: 'A filesystem I/O operation failed.',
	ffmpeg_error: 'The native FFmpeg pipeline reported an error.',
	process_termination_failed: 'The external processing process failed.',
	temp_directory_creation_failed: 'The processing workspace could not be created.',
	resource_cleanup_failed: 'Temporary resource cleanup failed.',
	internal_error: 'The processing pipeline failed internally.',
	image_processing_error: 'Cover-art processing failed.',
	processing_cancelled: 'Processing was cancelled.',
	toolchain_required: 'The selected encoder toolchain is unavailable or not configured.',
};

function failureExplanation(code: string | undefined): string | undefined {
	return code ? FAILURE_EXPLANATIONS[code] : undefined;
}

function highSignalLine(line: string): boolean {
	if (line.includes('RUST_LOG:')) return false;
	return /work_operation |processing_job |\b(?:ERROR|WARN)\b|Internal server error|panicked|panic|could not compile|failed to compile|exited with code/i.test(
		line,
	);
}

export function analyzeDevLog(
	mainLog: string,
	encodingLog: string,
	wrapperExitStatus: number,
): DevLogAnalysis {
	const cleanLog = stripAnsi(mainLog);
	const cleanEncodingLog = stripAnsi(encodingLog);
	const lines = cleanLog.split('\n');
	const operations = new Map<string, MutableOperation>();
	const jobs = new Map<string, MutableJob>();
	let malformedLifecycleLines = 0;

	for (const line of lines) {
		const operationFields = parseRecord(line, 'work_operation');
		if (operationFields) {
			const operation = operationFromFields(operationFields);
			if (!operation || !mergeOperation(operations, operation)) {
				malformedLifecycleLines += 1;
			}
		}

		const jobFields = parseRecord(line, 'processing_job');
		if (jobFields) {
			const job = jobFromFields(jobFields);
			if (!job || !mergeJob(jobs, job)) {
				malformedLifecycleLines += 1;
			}
		} else {
			const legacyResult = parseLegacyJobLine(line, jobs);
			if (legacyResult === 'malformed') malformedLifecycleLines += 1;
		}
	}

	const operationOutcomes = sortedValues(operations);
	const jobOutcomes = sortedValues(jobs);
	const unmatchedOperationIds = operationOutcomes
		.filter((operation) => operation.sawStart && !operation.sawTerminal)
		.map((operation) => operation.id);
	const unmatchedJobIds = jobOutcomes
		.filter((job) => job.sawStart && !job.sawTerminal)
		.map((job) => job.id);
	const orphanOperationIds = operationOutcomes
		.filter((operation) => !operation.sawStart)
		.map((operation) => operation.id);
	const orphanJobIds = jobOutcomes
		.filter((job) => job.sawTerminal && !job.sawStart)
		.map((job) => job.id);
	const appStarts = countMatches(cleanLog, /Starting AudioBook Boss application/g);
	const appRestarts = Math.max(appStarts - 1, 0);
	const viteRestarts = countMatches(cleanLog, /\[vite\] server restarted\./g);
	const viteInternalErrors = countMatches(cleanLog, /Internal server error/g);
	const rustErrors = countMatches(cleanLog, /\bERROR\b/g);
	const warningLines = lines.filter((line) => /\bWARN\b/.test(line));
	const rustWarnings = warningLines.length;
	const hasCancelledJob = jobOutcomes.some((job) => job.sawTerminal && job.status === 'cancelled');
	const actionableRustWarnings = warningLines.filter(
		(line) => !isExpectedCancellationWarning(line, hasCancelledJob),
	).length;
	const panics = countMatches(cleanLog, /thread .* panicked|\bpanicked\b|fatal runtime error/gi);
	const compilerFailures = countMatches(
		cleanLog,
		/could not compile|failed to compile|error\[E\d+\]|Failed to build application/gi,
	);
	const childExitCodes = [...cleanLog.matchAll(/exited with code (\d+)/g)].map((match) =>
		Number.parseInt(match[1], 10),
	);
	const hardChildExitCodes = childExitCodes.filter((code) => !EXPECTED_SIGNAL_EXIT_CODES.has(code));
	const failedOperations = operationOutcomes.filter(
		(operation) => operation.sawTerminal && (operation.status === 'failed' || operation.failed > 0),
	);
	const failedJobs = jobOutcomes.filter((job) => job.sawTerminal && job.status === 'failed');
	const { runs: externalFdkRunRecords, malformedRuns: malformedExternalFdkRuns } =
		parseExternalFdkRuns(cleanEncodingLog);
	const externalFdkStatuses: Record<string, number> = {};
	for (const run of externalFdkRunRecords) {
		if (!run.status) continue;
		externalFdkStatuses[run.status] = (externalFdkStatuses[run.status] ?? 0) + 1;
	}
	const failedExternalFdkRuns = externalFdkRunRecords.filter(
		(run) => run.status === 'failed' || run.status === 'wait_error',
	);
	const cancelledJobIds = new Set(
		jobOutcomes.filter((job) => job.sawTerminal && job.status === 'cancelled').map((job) => job.id),
	);
	const unexpectedExternalFdkInterruptions = externalFdkRunRecords.filter(
		(run) => run.status === 'interrupted' && (!run.jobId || !cancelledJobIds.has(run.jobId)),
	);
	const wrapperFailed =
		wrapperExitStatus !== 0 && !EXPECTED_SIGNAL_EXIT_CODES.has(wrapperExitStatus);
	const reasons: string[] = [];
	let health: SessionHealth;

	if (
		wrapperFailed ||
		hardChildExitCodes.length > 0 ||
		panics > 0 ||
		compilerFailures > 0 ||
		failedOperations.length > 0 ||
		failedJobs.length > 0 ||
		failedExternalFdkRuns.length > 0
	) {
		health = 'failed';
		if (wrapperFailed) reasons.push(`Wrapper exited with status ${wrapperExitStatus}.`);
		if (hardChildExitCodes.length > 0) {
			reasons.push(`Child process exited non-zero: ${hardChildExitCodes.join(', ')}.`);
		}
		if (panics > 0) reasons.push(`${panics} panic diagnostic(s) detected.`);
		if (compilerFailures > 0) reasons.push(`${compilerFailures} compiler failure(s) detected.`);
		for (const operation of failedOperations) {
			reasons.push(`Work operation ${operation.id} terminalized as ${operation.status}.`);
		}
		for (const job of failedJobs) {
			const typed = job.code ? ` (${job.code}/${job.category ?? 'unknown'})` : '';
			const explanation = failureExplanation(job.code);
			reasons.push(
				`Processing job ${job.id} failed${typed}.${explanation ? ` ${explanation}` : ''}`,
			);
		}
		for (const run of failedExternalFdkRuns) {
			const job = run.jobId ? ` for processing job ${run.jobId}` : '';
			reasons.push(`External FDK encoder reported ${run.status}${job}.`);
		}
	} else if (
		malformedLifecycleLines > 0 ||
		orphanOperationIds.length > 0 ||
		orphanJobIds.length > 0 ||
		malformedExternalFdkRuns > 0 ||
		appStarts === 0
	) {
		health = 'indeterminate';
		if (malformedLifecycleLines > 0) {
			reasons.push(`${malformedLifecycleLines} lifecycle record(s) violated the log contract.`);
		}
		for (const id of orphanOperationIds) {
			reasons.push(`Work operation ${id} has lifecycle records without an accepted record.`);
		}
		for (const id of orphanJobIds) {
			reasons.push(`Processing job ${id} has a terminal record without a start record.`);
		}
		if (malformedExternalFdkRuns > 0) {
			reasons.push(
				`${malformedExternalFdkRuns} external FDK run record(s) violated the encoding-log contract.`,
			);
		}
		if (appStarts === 0) reasons.push('No application startup record was captured.');
	} else if (
		unmatchedOperationIds.length > 0 ||
		unmatchedJobIds.length > 0 ||
		unexpectedExternalFdkInterruptions.length > 0
	) {
		health = 'interrupted';
		for (const id of unmatchedOperationIds) {
			reasons.push(`Work operation ${id} started without a terminal record.`);
		}
		for (const id of unmatchedJobIds) {
			reasons.push(`Processing job ${id} started without a terminal record.`);
		}
		for (const run of unexpectedExternalFdkInterruptions) {
			const job = run.jobId ? ` for processing job ${run.jobId}` : '';
			reasons.push(`External FDK encoder was interrupted${job}.`);
		}
	} else if (
		appRestarts > 0 ||
		viteRestarts > 0 ||
		viteInternalErrors > 0 ||
		rustErrors > 0 ||
		actionableRustWarnings > 0
	) {
		health = 'degraded';
		if (appRestarts > 0) reasons.push(`${appRestarts} application restart(s) detected.`);
		if (viteRestarts > 0) reasons.push(`${viteRestarts} Vite server restart(s) detected.`);
		if (viteInternalErrors > 0) {
			reasons.push(`${viteInternalErrors} Vite internal server error(s) detected.`);
		}
		if (rustErrors > 0) reasons.push(`${rustErrors} backend error log line(s) detected.`);
		if (actionableRustWarnings > 0) {
			reasons.push(`${actionableRustWarnings} actionable backend warning line(s) detected.`);
		}
	} else {
		health = 'clean';
		reasons.push('No unfinished lifecycle or hard diagnostic was detected.');
	}

	const encodingLines =
		cleanEncodingLog.trim().length === 0 ? 0 : cleanEncodingLog.trimEnd().split('\n').length;
	const externalFdkRuns = externalFdkRunRecords.length;
	const highSignalLines = lines
		.map((line, index) => ({ line, number: index + 1 }))
		.filter(({ line }) => highSignalLine(line))
		.slice(-40)
		.map(({ line, number }) => `${number}:${line}`);

	return {
		health,
		reasons,
		operations: operationOutcomes.map(
			({ sawStart: _sawStart, sawTerminal: _sawTerminal, ...operation }) => operation,
		),
		jobs: jobOutcomes.map(({ sawStart: _sawStart, sawTerminal: _sawTerminal, ...job }) => job),
		unmatchedOperationIds,
		unmatchedJobIds,
		orphanOperationIds,
		orphanJobIds,
		wrapperExitStatus,
		appStarts,
		appRestarts,
		viteRestarts,
		viteInternalErrors,
		rustErrors,
		rustWarnings,
		actionableRustWarnings,
		panics,
		compilerFailures,
		malformedLifecycleLines,
		childExitCodes,
		encoderLines: encodingLines,
		externalFdkRuns,
		externalFdkStatuses,
		malformedExternalFdkRuns,
		highSignalLines,
	};
}

function tableCell(value: string | number | undefined): string {
	return String(value ?? '—').replaceAll('|', '\\|');
}

function renderOperations(operations: OperationOutcome[]): string[] {
	if (operations.length === 0) return ['No Work Operation lifecycle records were captured.'];
	return [
		'| Operation | Kind | Status | Succeeded | Skipped | Cancelled | Failed |',
		'| --- | --- | --- | ---: | ---: | ---: | ---: |',
		...operations.map((operation) =>
			[
				`| \`${tableCell(operation.id)}\``,
				tableCell(operation.kind),
				operation.terminal ? tableCell(operation.status) : 'interrupted',
				tableCell(operation.succeeded),
				tableCell(operation.skipped),
				tableCell(operation.cancelled),
				`${tableCell(operation.failed)} |`,
			].join(' | '),
		),
	];
}

function renderJobs(jobs: JobOutcome[]): string[] {
	if (jobs.length === 0) return ['No processing-job lifecycle records were captured.'];
	return [
		'| Job | Operation | Input | Kind | Status | Elapsed | Error |',
		'| --- | --- | ---: | --- | --- | ---: | --- |',
		...jobs.map((job) => {
			const typedError = job.code ? `${job.code}/${job.category ?? 'unknown'}` : undefined;
			const explanation = failureExplanation(job.code);
			const error = [typedError, explanation].filter(Boolean).join(' — ') || '—';
			return [
				`| \`${tableCell(job.id)}\``,
				tableCell(job.operationId),
				tableCell(job.inputIndex),
				tableCell(job.kind),
				job.terminal ? tableCell(job.status) : 'interrupted',
				job.elapsedMs === undefined ? '—' : `${job.elapsedMs}ms`,
				`${tableCell(error)} |`,
			].join(' | ');
		}),
	];
}

export function renderDevLogAnalysis(analysis: DevLogAnalysis): string {
	const childExits =
		analysis.childExitCodes.length > 0 ? analysis.childExitCodes.join(', ') : 'none';
	const encoderStatuses = Object.entries(analysis.externalFdkStatuses)
		.map(([status, count]) => `${status}=${count}`)
		.join(' ');
	return [
		'## Session Verdict',
		'',
		`- Health: \`${analysis.health}\``,
		...analysis.reasons.map((reason) => `- ${reason}`),
		'',
		'## Work Operations',
		'',
		...renderOperations(analysis.operations),
		'',
		'## Processing Jobs',
		'',
		...renderJobs(analysis.jobs),
		'',
		'## Runtime / Build Diagnostics',
		'',
		'```text',
		`wrapper_exit=${analysis.wrapperExitStatus}`,
		`app_starts=${analysis.appStarts} app_restarts=${analysis.appRestarts} vite_restarts=${analysis.viteRestarts}`,
		`vite_internal_errors=${analysis.viteInternalErrors}`,
		`rust_errors=${analysis.rustErrors} rust_warnings=${analysis.rustWarnings} actionable_rust_warnings=${analysis.actionableRustWarnings}`,
		`panics=${analysis.panics} compiler_failures=${analysis.compilerFailures}`,
		`malformed_lifecycle_lines=${analysis.malformedLifecycleLines} unaccepted_operations=${analysis.orphanOperationIds.length} orphan_jobs=${analysis.orphanJobIds.length}`,
		`child_exit_codes=${childExits}`,
		'```',
		'',
		'## Recent High-Signal Lines',
		'',
		'```text',
		...(analysis.highSignalLines.length > 0 ? analysis.highSignalLines : ['none']),
		'```',
		'',
		'## Encoder Log',
		'',
		'```text',
		`lines=${analysis.encoderLines} external_fdk_runs=${analysis.externalFdkRuns} malformed_external_fdk_runs=${analysis.malformedExternalFdkRuns}${encoderStatuses ? ` ${encoderStatuses}` : ''}`,
		'```',
		'',
	].join('\n');
}

interface CliOptions {
	mainLog: string;
	encodingLog: string;
	exitStatus: number;
}

function cliOptions(args: string[]): CliOptions {
	const values = new Map<string, string>();
	for (let index = 0; index < args.length; index += 2) {
		const flag = args[index];
		const value = args[index + 1];
		if (!flag?.startsWith('--') || value === undefined) {
			throw new Error(
				'Usage: bun scripts/dev-log-analysis.ts --main-log <path> --encoding-log <path> --exit-status <code>',
			);
		}
		values.set(flag, value);
	}
	const mainLog = values.get('--main-log');
	const encodingLog = values.get('--encoding-log');
	const exitStatus = Number.parseInt(values.get('--exit-status') ?? '', 10);
	if (!mainLog || !encodingLog || !Number.isFinite(exitStatus)) {
		throw new Error(
			'Usage: bun scripts/dev-log-analysis.ts --main-log <path> --encoding-log <path> --exit-status <code>',
		);
	}
	return { mainLog, encodingLog, exitStatus };
}

function main(args: string[]): void {
	const options = cliOptions(args);
	const analysis = analyzeDevLog(
		readFileSync(options.mainLog, 'utf8'),
		readFileSync(options.encodingLog, 'utf8'),
		options.exitStatus,
	);
	process.stdout.write(renderDevLogAnalysis(analysis));
}

if (import.meta.main) {
	main(process.argv.slice(2));
}
