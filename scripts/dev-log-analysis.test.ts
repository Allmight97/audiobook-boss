import { describe, expect, it } from 'vitest';

import { analyzeDevLog, renderDevLogAnalysis, stripAnsi } from './dev-log-analysis';

const APP_START =
	'[2026-07-10T15:15:21Z INFO  audiobook_boss_lib] Starting AudioBook Boss application';

function externalFdkRun(status: string, jobId?: string): string {
	return [
		'--- external-fdk run 1783700000 ---',
		'run_id=test-run',
		`status=${status}`,
		...(jobId ? [`job_id=${jobId}`] : []),
		'--- end external-fdk run ---',
	].join('\n');
}

describe('analyzeDevLog', () => {
	it('classifies a completed operation and job as clean', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'work_operation event=accepted operation_id=op-1 kind=processing_batch status=accepted total=1 succeeded=0 skipped=0 cancelled=0 failed=0',
				'work_operation event=running operation_id=op-1 kind=processing_batch status=running total=1 succeeded=0 skipped=0 cancelled=0 failed=0',
				'processing_job event=started operation_id=op-1 job_id=job-1 input_index=0 kind=processing_batch status=running',
				'processing_job event=terminal operation_id=op-1 job_id=job-1 input_index=0 kind=processing_batch status=success elapsed_ms=42',
				'work_operation event=terminal operation_id=op-1 kind=processing_batch status=completed total=1 succeeded=1 skipped=0 cancelled=0 failed=0',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('clean');
		expect(analysis.unmatchedOperationIds).toEqual([]);
		expect(analysis.unmatchedJobIds).toEqual([]);
		expect(analysis.operations[0]).toMatchObject({
			id: 'op-1',
			status: 'completed',
			terminal: true,
		});
		expect(analysis.jobs[0]).toMatchObject({ id: 'job-1', status: 'success', elapsedMs: 42 });
	});

	it('treats cancellation as terminal truth rather than failure', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'work_operation event=accepted operation_id=op-cancel kind=processing_batch status=accepted total=1 succeeded=0 skipped=0 cancelled=0 failed=0',
				'processing_job event=started operation_id=op-cancel job_id=job-cancel input_index=0 kind=processing_batch status=running',
				'processing_job event=terminal operation_id=op-cancel job_id=job-cancel input_index=0 kind=processing_batch status=cancelled elapsed_ms=12',
				'work_operation event=terminal operation_id=op-cancel kind=processing_batch status=cancelled total=1 succeeded=0 skipped=0 cancelled=1 failed=0',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('clean');
		expect(analysis.jobs[0].status).toBe('cancelled');
	});

	it('classifies recovered Vite errors and restarts as degraded', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'8:25:50 AM [vite] Internal server error: Invalid declaration',
				'8:25:55 AM [vite] server restarted.',
				APP_START,
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('degraded');
		expect(analysis.appStarts).toBe(2);
		expect(analysis.appRestarts).toBe(1);
		expect(analysis.viteRestarts).toBe(1);
		expect(analysis.viteInternalErrors).toBe(1);
	});

	it('keeps a terminal mixed outcome clean when no item failed', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'work_operation event=accepted operation_id=op-mixed kind=processing_batch status=accepted total=2 succeeded=0 skipped=0 cancelled=0 failed=0',
				'work_operation event=terminal operation_id=op-mixed kind=processing_batch status=mixed total=2 succeeded=1 skipped=1 cancelled=0 failed=0',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('clean');
		expect(analysis.operations[0]).toMatchObject({ status: 'mixed', failed: 0, terminal: true });
	});

	it('classifies typed terminal failures as failed', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'processing_job event=started operation_id=op-2 job_id=job-2 input_index=3 kind=processing_batch status=running',
				'processing_job event=terminal operation_id=op-2 job_id=job-2 input_index=3 kind=processing_batch status=failed elapsed_ms=9 code=ffmpeg_error category=toolchain',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('failed');
		expect(analysis.reasons).toContain(
			'Processing job job-2 failed (ffmpeg_error/toolchain). The native FFmpeg pipeline reported an error.',
		);
		expect(renderDevLogAnalysis(analysis)).toContain(
			'ffmpeg_error/toolchain — The native FFmpeg pipeline reported an error.',
		);
	});

	it('keeps an unknown typed error visible without inventing an explanation', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'processing_job event=started operation_id=op-new job_id=job-new input_index=0 kind=processing_batch status=running',
				'processing_job event=terminal operation_id=op-new job_id=job-new input_index=0 kind=processing_batch status=failed elapsed_ms=4 code=future_failure category=future',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('failed');
		expect(renderDevLogAnalysis(analysis)).toContain('future_failure/future');
	});

	it.each([
		{ name: 'wrapper failure', log: APP_START, exitStatus: 1 },
		{ name: 'panic', log: `${APP_START}\nthread main panicked at source.rs:10`, exitStatus: 0 },
		{
			name: 'compiler failure',
			log: `${APP_START}\nerror[E0308]: mismatched types`,
			exitStatus: 0,
		},
	])('classifies $name as failed', ({ log, exitStatus }) => {
		expect(analyzeDevLog(log, '', exitStatus).health).toBe('failed');
	});

	it('classifies unmatched lifecycle starts as interrupted', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'work_operation event=accepted operation_id=op-3 kind=processing_merge status=accepted total=1 succeeded=0 skipped=0 cancelled=0 failed=0',
				'processing_job event=started operation_id=op-3 job_id=job-3 input_index=none kind=processing_merge status=running',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('interrupted');
		expect(analysis.unmatchedOperationIds).toEqual(['op-3']);
		expect(analysis.unmatchedJobIds).toEqual(['job-3']);
	});

	it('classifies orphan terminal records as indeterminate', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'processing_job event=terminal operation_id=op-orphan job_id=job-orphan input_index=0 kind=processing_batch status=success elapsed_ms=4',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('indeterminate');
		expect(analysis.orphanJobIds).toEqual(['job-orphan']);
	});

	it('requires an accepted operation record before later lifecycle events', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'work_operation event=running operation_id=op-unaccepted kind=processing_batch status=running total=1 succeeded=0 skipped=0 cancelled=0 failed=0',
				'work_operation event=terminal operation_id=op-unaccepted kind=processing_batch status=completed total=1 succeeded=1 skipped=0 cancelled=0 failed=0',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('indeterminate');
		expect(analysis.orphanOperationIds).toEqual(['op-unaccepted']);
	});

	it.each([
		'processing_job event=terminal operation_id=op-bad job_id=job-bad input_index=0 kind=processing_batch status=unknown elapsed_ms=4',
		'processing_job event=finished operation_id=op-bad job_id=job-bad input_index=0 kind=processing_batch status=success elapsed_ms=4',
		'work_operation event=terminal operation_id=op-bad kind=processing_batch status=unknown total=1 succeeded=1 skipped=0 cancelled=0 failed=0',
	])('classifies an invalid lifecycle record as indeterminate: %s', (record) => {
		const analysis = analyzeDevLog(`${APP_START}\n${record}`, '', 0);

		expect(analysis.health).toBe('indeterminate');
		expect(analysis.malformedLifecycleLines).toBe(1);
	});

	it('rejects typed failure fields on a successful job', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'processing_job event=started operation_id=op-bad-fields job_id=job-bad-fields input_index=0 kind=processing_batch status=running',
				'processing_job event=terminal operation_id=op-bad-fields job_id=job-bad-fields input_index=0 kind=processing_batch status=success elapsed_ms=4 code=ffmpeg_error category=toolchain',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('indeterminate');
		expect(analysis.malformedLifecycleLines).toBe(1);
	});

	it('degrades on actionable backend warnings', () => {
		const analysis = analyzeDevLog(
			`${APP_START}\n[WARN audiobook_boss_lib] output_parent_cleanup status=terminal_cleanup_err err=permission denied`,
			'',
			0,
		);

		expect(analysis.health).toBe('degraded');
		expect(analysis.rustWarnings).toBe(1);
		expect(analysis.actionableRustWarnings).toBe(1);
	});

	it('does not degrade an otherwise clean cancellation for its expected warning', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'processing_job event=started operation_id=op-cancel job_id=job-cancel input_index=0 kind=processing_batch status=running',
				'[WARN audiobook_boss_lib] processing_job event=terminal operation_id=op-cancel job_id=job-cancel input_index=0 kind=processing_batch status=cancelled elapsed_ms=12',
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('clean');
		expect(analysis.rustWarnings).toBe(1);
		expect(analysis.actionableRustWarnings).toBe(0);
	});

	it.each(['failed', 'wait_error'])('classifies external FDK status=%s as failed', (status) => {
		const analysis = analyzeDevLog(APP_START, externalFdkRun(status, 'job-fdk'), 0);

		expect(analysis.health).toBe('failed');
		expect(analysis.externalFdkStatuses).toMatchObject({ [status]: 1 });
	});

	it('classifies an uncorrelated external FDK interruption as interrupted', () => {
		const analysis = analyzeDevLog(APP_START, externalFdkRun('interrupted', 'job-fdk'), 0);

		expect(analysis.health).toBe('interrupted');
	});

	it('accepts an external FDK interruption correlated to a cancelled job', () => {
		const analysis = analyzeDevLog(
			[
				APP_START,
				'processing_job event=started operation_id=op-fdk job_id=job-fdk input_index=0 kind=processing_batch status=running',
				'[WARN audiobook_boss_lib] processing_job event=terminal operation_id=op-fdk job_id=job-fdk input_index=0 kind=processing_batch status=cancelled elapsed_ms=12',
			].join('\n'),
			externalFdkRun('interrupted', 'job-fdk'),
			0,
		);

		expect(analysis.health).toBe('clean');
	});

	it('classifies an unknown external FDK status as indeterminate', () => {
		const analysis = analyzeDevLog(APP_START, externalFdkRun('mystery'), 0);

		expect(analysis.health).toBe('indeterminate');
		expect(analysis.malformedExternalFdkRuns).toBe(1);
	});

	it('does not parse raw external FDK stderr as structured fields', () => {
		const encodingLog = [
			'--- external-fdk run 1783700000 ---',
			'run_id=test-run',
			'status=success',
			'job_id=job-real',
			'stderr:',
			'status=failed',
			'job_id=job-spoofed',
			'--- end external-fdk run ---',
		].join('\n');
		const analysis = analyzeDevLog(APP_START, encodingLog, 0);

		expect(analysis.health).toBe('clean');
		expect(analysis.externalFdkStatuses).toEqual({ success: 1 });
		expect(analysis.malformedExternalFdkRuns).toBe(0);
	});

	it('does not treat normal signal-based dev shutdown as failure', () => {
		const analysis = analyzeDevLog(
			`${APP_START}\nerror: script "dev" exited with code 143`,
			'',
			143,
		);

		expect(analysis.health).toBe('clean');
		expect(analysis.childExitCodes).toEqual([143]);
	});

	it('strips ANSI before recognizing lifecycle records', () => {
		const escapeCode = String.fromCharCode(27);
		const colored = `${escapeCode}[32m${APP_START}${escapeCode}[0m\n${escapeCode}[31mprocessing_job event=started operation_id=foreground job_id=job-ansi input_index=none kind=processing_merge status=running${escapeCode}[0m`;
		const analysis = analyzeDevLog(colored, '', 0);

		expect(stripAnsi(colored)).not.toContain(escapeCode);
		expect(analysis.health).toBe('interrupted');
		expect(analysis.unmatchedJobIds).toEqual(['job-ansi']);
	});

	it('strips terminal hyperlink sequences without removing their label', () => {
		const escapeCode = String.fromCharCode(27);
		const linked = `${escapeCode}]8;;https://doc.rust-lang.org${escapeCode}\\dev${escapeCode}]8;;${escapeCode}\\ profile`;

		expect(stripAnsi(linked)).toBe('dev profile');
	});

	it('is indeterminate when lifecycle evidence is malformed or startup is absent', () => {
		const analysis = analyzeDevLog('work_operation event=accepted status=accepted', '', 0);

		expect(analysis.health).toBe('indeterminate');
		expect(analysis.malformedLifecycleLines).toBe(1);
		expect(analysis.reasons).toContain('No application startup record was captured.');
	});

	it('replays the captured legacy interruption and names the unmatched job', () => {
		const jobId = 'c4e62f2a-c5c3-4117-9c54-1e4d9da15131';
		const analysis = analyzeDevLog(
			[
				APP_START,
				`[INFO] Job ${jobId} registered and started`,
				`[INFO] Job ${jobId} started for output: /Volumes/data/Book.m4b`,
				'8:25:50 AM [vite] Internal server error: Invalid declaration',
				'8:25:55 AM [vite] server restarted.',
				APP_START,
				'error: script "dev" exited with code 143',
			].join('\n'),
			'# ABB encoding log\nrun_id=legacy\n',
			0,
		);

		expect(analysis.health).toBe('interrupted');
		expect(analysis.unmatchedJobIds).toEqual([jobId]);
		expect(renderDevLogAnalysis(analysis)).toContain(jobId);
	});

	it('pairs completed legacy start and terminal records', () => {
		const jobId = 'c4e62f2a-c5c3-4117-9c54-1e4d9da15131';
		const analysis = analyzeDevLog(
			[
				APP_START,
				`[INFO] Job ${jobId} started for output: /Volumes/data/Book.m4b`,
				`[INFO] Job ${jobId} completed successfully`,
			].join('\n'),
			'',
			0,
		);

		expect(analysis.health).toBe('clean');
		expect(analysis.jobs[0]).toMatchObject({ id: jobId, status: 'success', terminal: true });
	});
});
