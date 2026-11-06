import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppErrorEnvelope } from '../generated/tauri';
import type { tauriClient as TauriClient } from './client';
import { recordedFresh } from './recordedFresh';

describe('tauriClient', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('sends absent remembered defaults as null and returns the settings in effect', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./client');
		vi.mocked(invoke).mockResolvedValueOnce({
			outcome: { kind: 'applied' },
			snapshot: {
				revision: 3,
				settings: null,
				saveError: null,
				concurrency: {
					preference: { mode: 'auto' },
					effective: 4,
					capabilities: recordedFresh.settings.concurrency.capabilities,
				},
				startupDefaults: null,
				recovery: null,
			},
		});

		const reply = await tauriClient.settingsDispatch(7, 2, {
			kind: 'remember',
			defaultAcquisitionLane: 'indexer',
		});

		expect(invoke).toHaveBeenLastCalledWith('settings_dispatch', {
			client: 7,
			sequence: 2,
			intent: {
				kind: 'remember',
				encoderDefaults: null,
				outputDefaults: null,
				defaultAcquisitionLane: 'indexer',
			},
		});
		expect(reply.outcome).toEqual({ kind: 'applied' });
		expect(reply.snapshot.settings).toBeUndefined();
		expect(reply.snapshot.recovery).toBeUndefined();
	});

	it('passes audio requests through and drops nulls from audio files', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./client');
		const passThrough = { format: 'mp3', intent: 'preserve', sampleRate: 'auto' };
		vi.mocked(invoke).mockResolvedValueOnce({
			outcome: { kind: 'applied' },
			update: {
				revision: 4,
				titles: {
					revision: 4,
					files: [{ inputId: 'a', path: '/books/a.mp3', isValid: true, error: null, size: null }],
					titleSourcesByIdentity: {},
					audioChoiceRequired: [],
					sortDirection: 'none',
					orderLocked: false,
					notice: null,
					orderDiffersFromImport: false,
					companions: {},
				},
				selection: null,
				metadata: null,
				lookup: null,
				audio: {
					revision: 4,
					capabilities: null,
					defaults: { choice: {}, facts: {}, request: passThrough },
					titles: {},
				},
			},
		});

		const reply = await tauriClient.sessionDispatch(7, 0, { kind: 'selectAll' });

		expect(invoke).toHaveBeenLastCalledWith('session_dispatch', {
			client: 7,
			sequence: 0,
			intent: { kind: 'selectAll' },
		});
		const titles = reply.update.titles;
		expect(titles?.files[0]).toEqual({ inputId: 'a', path: '/books/a.mp3', isValid: true });
		expect(reply.update.audio?.defaults.request).toEqual(passThrough);
		expect(reply.update).not.toHaveProperty('selection', null);
		expect(reply.update.selection).toBeUndefined();
	});

	it('carries identified decisions and preserves nullable held-question facts', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./client');
		vi.mocked(invoke).mockResolvedValueOnce({
			outcome: { kind: 'superseded' },
			update: {
				revision: 4,
				output: {
					revision: 4,
					directory: null,
					preset: 'absDefault',
					includeYear: false,
					template: '',
					naming: { preset: 'absDefault', includeYear: false, customTemplate: null },
					preview: { kind: 'noDirectory' },
					submission: null,
					submissionInProgress: true,
					collisionReview: {
						reviewId: 42,
						outputs: [
							{
								inputIndex: 0,
								inputPath: '/source.m4b',
								kind: 'final',
								requestedPath: '/out.m4b',
								resolvedPath: '/out.m4b',
								renameCandidate: null,
								collision: null,
								action: 'review_required',
								review: null,
							},
						],
					},
					restartOffers: [],
					restartPrompt: null,
					previewRun: null,
				},
			},
		});
		const reply = await tauriClient.sessionDispatch(7, 2, {
			kind: 'chooseCollisionPolicy',
			reviewId: 41,
			policy: 'replace_existing',
		});
		expect(invoke).toHaveBeenLastCalledWith('session_dispatch', {
			client: 7,
			sequence: 2,
			intent: { kind: 'chooseCollisionPolicy', reviewId: 41, policy: 'replace_existing' },
		});
		expect(reply.outcome).toEqual({ kind: 'superseded' });
		expect(reply.update.output?.collisionReview?.reviewId).toBe(42);
		expect(reply.update.output?.collisionReview?.outputs[0].renameCandidate).toBeUndefined();
		expect(reply.update.output?.restartPrompt).toBeNull();
		expect(reply.update.output?.submissionInProgress).toBe(true);
	});

	describe('native dialog and opener helpers', () => {
		it.each([
			{
				call: (client: typeof TauriClient) => client.openFile({ title: 'Select file' }),
				picked: '/tmp/book.m4b',
				options: { title: 'Select file', multiple: false, directory: false },
			},
			{
				call: (client: typeof TauriClient) => client.openFiles(),
				picked: ['/tmp/a.m4b', '/tmp/b.m4b'],
				options: { multiple: true, directory: false },
			},
			{
				call: (client: typeof TauriClient) => client.openDirectory(),
				picked: '/tmp/output',
				options: { multiple: false, directory: true },
			},
		])(
			'opens the native dialog in the mode each picker asks for',
			async ({ call, picked, options }) => {
				const { open } = await import('@tauri-apps/plugin-dialog');
				const mockOpen = vi.mocked(open);
				mockOpen.mockResolvedValueOnce(picked);
				const { tauriClient } = await import('./client');

				await expect(call(tauriClient)).resolves.toEqual(picked);

				expect(mockOpen).toHaveBeenLastCalledWith(options);
			},
		);

		it('routes URLs to the Tauri opener', async () => {
			const { openUrl } = await import('@tauri-apps/plugin-opener');
			const mockOpenUrl = vi.mocked(openUrl);

			const { tauriClient } = await import('./client');
			await tauriClient.openUrl('https://example.com/login');

			expect(mockOpenUrl).toHaveBeenLastCalledWith('https://example.com/login');
		});
	});
});

describe('tauriClient nullish adapters', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
	});

	it('normalizes a finished preview carried in the session output', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		vi.mocked(invoke).mockResolvedValueOnce({
			outcome: { kind: 'applied' },
			update: {
				revision: 5,
				output: {
					revision: 5,
					directory: '/tmp/out',
					preset: 'absDefault',
					includeYear: false,
					template: '',
					naming: { preset: 'absDefault', includeYear: false, customTemplate: null },
					preview: { kind: 'noTitle' },
					submission: {
						kind: 'previewFinished',
						result: {
							summary: { total: 2, succeeded: 1, skipped: 0, cancelled: 0, failed: 1 },
							terminalClass: 'mixed',
							results: [
								{ inputIndex: 0, status: 'success', message: 'ok', jobId: 'job-1', error: null },
								{
									inputIndex: 1,
									status: 'failed',
									message: 'failed',
									jobId: null,
									error: {
										code: 'ffmpeg_error',
										category: 'toolchain',
										message: 'decoder unavailable',
										detail: 'ffmpeg missing',
									},
								},
							],
						},
					},
				},
			},
		});

		const { tauriClient } = await import('./client');
		const reply = await tauriClient.sessionDispatch(7, 0, { kind: 'preview' });

		const submission = reply.update.output?.submission;
		expect(submission?.kind).toBe('previewFinished');
		if (submission?.kind !== 'previewFinished') return;
		expect(submission.result.results[0]).toEqual({
			inputIndex: 0,
			status: 'success',
			message: 'ok',
			jobId: 'job-1',
		});
		expect(submission.result.results[1]?.error?.message).toBe('decoder unavailable');
		expect(submission.result.results[1]?.jobId).toBeUndefined();
	});

	it('unwraps generated Result error responses into normalized app errors', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockRejectedValueOnce({
			code: 'processing_cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled',
			detail: 'user requested stop',
		} satisfies AppErrorEnvelope);

		const { tauriClient } = await import('./client');

		await expect(tauriClient.cancelWorkOperation('mock-operation-1')).rejects.toMatchObject({
			code: 'processing_cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled',
			detail: 'user requested stop',
		});
	});

	it('shows the fallback text when an error field is an object', async () => {
		const { normalizeAppError } = await import('./appError');
		expect(
			normalizeAppError({ code: { id: 7 }, category: 'io', message: { text: 'x' } }),
		).toMatchObject({
			code: 'unknown_error',
			category: 'io',
			message: 'Unknown error',
		});
	});
});
