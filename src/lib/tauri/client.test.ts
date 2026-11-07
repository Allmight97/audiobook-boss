import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppErrorEnvelope } from '../generated/tauri';
import type { tauriClient as TauriClient } from './client';
import { recordedFresh } from './recordedFresh';

describe('tauriClient', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('sends a settings intent as written and returns the settings in effect', async () => {
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
				recovery: null,
			},
		});

		const reply = await tauriClient.commands.settingsDispatch(7, 2, {
			kind: 'remember',
			encoderDefaults: null,
			outputDefaults: null,
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
		expect(reply.snapshot.settings).toBeNull();
		expect(reply.snapshot.recovery).toBeNull();
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
		const reply = await tauriClient.commands.sessionDispatch(7, 2, {
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

describe('tauriClient errors', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
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

		await expect(
			tauriClient.commands.cancelWorkOperation('mock-operation-1', null),
		).rejects.toMatchObject({
			code: 'processing_cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled',
			detail: 'user requested stop',
		});
	});

	it('never reads a refused command as success', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./client');
		const commands = Object.values(tauriClient.commands);
		const refusal = {
			code: 'processing_cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled',
			detail: 'user requested stop',
		} satisfies AppErrorEnvelope;
		for (let sent = 0; sent < commands.length; sent += 1) {
			vi.mocked(invoke).mockRejectedValueOnce(refusal);
		}

		await Promise.all(
			commands.map((command) =>
				expect(Promise.resolve(Reflect.apply(command, undefined, []))).rejects.toMatchObject({
					code: 'processing_cancelled',
				}),
			),
		);
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
