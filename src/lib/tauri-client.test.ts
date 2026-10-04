/**
 * Tests for the Tauri tauri client boundary.
 *
 * These tests verify boundary normalization and command/event wiring
 * against mocked Tauri APIs.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { runtimeSettingsCapabilitiesFixture } from '../test/fixtures/runtimeSettingsCapabilities';
import mainWindowCapability from '../../src-tauri/capabilities/default.json';

// Note: Tauri APIs are auto-mocked by src/test/setup.ts

describe('tauriClient', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('sends absent remembered defaults as null and returns the settings in effect', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./tauri/client');
		vi.mocked(invoke).mockResolvedValueOnce({
			outcome: { kind: 'applied' },
			snapshot: {
				revision: 3,
				settings: null,
				saveError: null,
				concurrency: {
					preference: { mode: 'auto' },
					effective: 4,
					capabilities: runtimeSettingsCapabilitiesFixture().maxConcurrentJobs,
				},
				startupDefaults: null,
				defaultAcquisitionLane: 'indexer',
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
		expect(reply.snapshot.defaultAcquisitionLane).toBe('indexer');
	});

	it('keeps explicit nulls in audio requests and drops them from audio files', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./tauri/client');
		const passThrough = { format: 'mp3', intent: 'preserve', settings: null, sampleRate: 'auto' };
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
		// A null `settings` is the request for MP3 pass-through.
		expect(reply.update.audio?.defaults.request).toEqual(passThrough);
		expect(reply.update).not.toHaveProperty('selection', null);
		expect(reply.update.selection).toBeUndefined();
	});

	it('carries identified decisions and preserves nullable held-question facts', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const { tauriClient } = await import('./tauri/client');
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
						preview: false,
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

	describe('dialog helpers', () => {
		it('sets single-file dialog options at the boundary', async () => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const mockOpen = vi.mocked(open);
			mockOpen.mockResolvedValueOnce('/tmp/book.m4b');

			const { tauriClient } = await import('./tauri/client');
			await expect(tauriClient.openFile({ title: 'Select file' })).resolves.toBe('/tmp/book.m4b');

			expect(mockOpen).toHaveBeenLastCalledWith({
				title: 'Select file',
				multiple: false,
				directory: false,
			});
		});

		it('sets multi-file dialog options at the boundary', async () => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const mockOpen = vi.mocked(open);
			mockOpen.mockResolvedValueOnce(['/tmp/a.m4b', '/tmp/b.m4b']);

			const { tauriClient } = await import('./tauri/client');
			await expect(tauriClient.openFiles()).resolves.toEqual(['/tmp/a.m4b', '/tmp/b.m4b']);

			expect(mockOpen).toHaveBeenLastCalledWith({ multiple: true, directory: false });
		});

		it('sets directory dialog options at the boundary', async () => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const mockOpen = vi.mocked(open);
			mockOpen.mockResolvedValueOnce('/tmp/output');

			const { tauriClient } = await import('./tauri/client');
			await expect(tauriClient.openDirectory()).resolves.toBe('/tmp/output');

			expect(mockOpen).toHaveBeenLastCalledWith({ multiple: false, directory: true });
		});
	});

	describe('opener helpers', () => {
		it('allows both web schemes used by source details without permitting other URL handlers', () => {
			expect(
				mainWindowCapability.permissions.find(
					(permission) =>
						typeof permission === 'object' && permission.identifier === 'opener:allow-open-url',
				),
			).toEqual({
				identifier: 'opener:allow-open-url',
				allow: [{ url: 'http://*' }, { url: 'https://*' }],
			});
		});

		it('limits source and preview opening to user-owned or mounted paths', () => {
			const openPathPermission = mainWindowCapability.permissions.find(
				(permission) =>
					typeof permission === 'object' && permission.identifier === 'opener:allow-open-path',
			);

			expect(openPathPermission).toEqual({
				identifier: 'opener:allow-open-path',
				allow: [{ path: '$HOME/**' }, { path: '$TEMP/**' }, { path: '/Volumes/**' }],
			});
		});

		it('routes paths and URLs to distinct Tauri opener commands', async () => {
			const { openPath, openUrl } = await import('@tauri-apps/plugin-opener');
			const mockOpenPath = vi.mocked(openPath);
			const mockOpenUrl = vi.mocked(openUrl);

			const { tauriClient } = await import('./tauri/client');
			await tauriClient.openPath('/tmp/preview.m4b');
			await tauriClient.openUrl('https://example.com/login');

			expect(mockOpenPath).toHaveBeenLastCalledWith('/tmp/preview.m4b', undefined);
			expect(mockOpenUrl).toHaveBeenLastCalledWith('https://example.com/login', undefined);
		});
	});
});

describe('tauriClient nullish adapters', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
	});

	it('normalizes nullable metadata fields from backend responses', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			title: 'Book A',
			artist: null,
			album: null,
			composer: null,
			genre: null,
			date: null,
			track: null,
			disk: null,
			comment: null,
			description: null,
			series: null,
			series_part: null,
			subseries: null,
			subseries_part: null,
			album_sort: null,
			cover_art: null,
		});

		const { tauriClient } = await import('./tauri/client');
		const metadata = await tauriClient.readAudioMetadata('/books/a.m4b');
		expect(metadata.title).toBe('Book A');
		expect(metadata.artist).toBeUndefined();
		expect(metadata.series).toBeUndefined();
		expect(metadata.cover_art).toBeUndefined();
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

		const { tauriClient } = await import('./tauri/client');
		const reply = await tauriClient.sessionDispatch(7, 0, { kind: 'preview', seconds: 30 });

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
			code: 'cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled.',
			detail: 'user requested stop',
		});

		const { tauriClient } = await import('./tauri/client');

		await expect(tauriClient.cancelWorkOperation('mock-operation-1')).rejects.toMatchObject({
			code: 'cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled.',
			detail: 'user requested stop',
		});
	});

	it('returns .data on canonical specta success shape', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		const result = unwrapGeneratedResult<{ hello: string }>({
			status: 'ok',
			data: { hello: 'world' },
		});
		expect(result).toEqual({ hello: 'world' });
	});

	it('throws normalized AppError on canonical specta error shape', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		expect(() =>
			unwrapGeneratedResult({
				status: 'error',
				error: {
					code: 'toolchain_missing',
					category: 'toolchain',
					message: 'ffmpeg not found',
				},
			}),
		).toThrow(
			expect.objectContaining({
				code: 'toolchain_missing',
				category: 'toolchain',
				message: 'ffmpeg not found',
			}) as unknown as Error,
		);
	});

	it('passes through bare scalar values unchanged (get_max_concurrent_jobs path)', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		expect(unwrapGeneratedResult<number>(42)).toBe(42);
	});

	it('passes through bare object values unchanged (EncoderAvailability path)', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		const bareEncoderAvailability = {
			ffmpegAvailable: true,
			ffprobeAvailable: true,
			availableEncoders: ['aac', 'libmp3lame'],
		};
		const result = unwrapGeneratedResult<typeof bareEncoderAvailability>(bareEncoderAvailability);
		expect(result).toBe(bareEncoderAvailability);
	});

	// Misclassification guard: without the discriminant check on `status === 'ok' | 'error'`,
	// a hypothetical future domain type with an unrelated `status` field (e.g. job state)
	// would be incorrectly stripped to its `.data`. This locks the safety property in place.
	it('passes through records with unrelated status values rather than treating them as Result', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		const bareDomainObject = { status: 'pending', data: { jobId: 'abc' } };
		const result = unwrapGeneratedResult<typeof bareDomainObject>(bareDomainObject);
		expect(result).toBe(bareDomainObject);
		expect(result.status).toBe('pending');
	});
});
