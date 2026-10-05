import { coverSrc } from '../../../lib/tauri/coverSrc';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@solidjs/testing-library';
import { AppRuntimeProvider } from '../../runtime';
import { StatusPanelView } from '../../../ui/statusPanel';
import { PreviewAudioControls } from '../../../ui/previewAudio';
import { tauriClient } from '../../../lib/tauri/client';
import { createFakeEngine } from '../../../test/fixtures/fakeEngine';
import type { SessionOutcome, SessionOutput } from '../../../types/session';
import { createAppRuntime, type AppRuntime } from '../../runtime';

function preview(): NonNullable<SessionOutput['previewRun']> {
	const progress = {
		stage: 'converting' as const,
		percentage: 42,
		message: 'Rendering excerpt.',
		currentItemIndex: 0,
		totalItems: 1,
		bytesDownloaded: null,
		bytesTotal: null,
		etaSeconds: 3,
	};
	return {
		openReady: false,
		artworkReady: true,
		operation: {
			operationId: 'preview-1',
			sequence: 0,
			revision: 2,
			kind: 'processingBatch',
			status: 'running',
			title: 'Edited book',
			createdAtMs: 0,
			startedAtMs: 1,
			finishedAtMs: null,
			cancellable: true,
			cancelRequested: false,
			lanes: ['encodeCpu'],
			sourceInputIds: ['title-1'],
			progress,
			terminalSummary: null,
			errors: [],
			logTail: [],
			children: [
				{
					childJobId: 'input-0',
					operationId: 'preview-1',
					label: 'Book.m4b',
					status: 'running',
					startedAtMs: 1,
					finishedAtMs: null,
					lane: 'encodeCpu',
					progress,
					sourcePath: '/books/Book.m4b',
					inputIndex: 0,
					inputId: 'title-1',
					sourceInputIds: ['title-1'],
					jobId: 'native-job',
					cancellable: true,
					cancelRequested: false,
					message: null,
					outputPath: null,
					supplementalWarning: null,
					outputUpdate: null,
				},
			],
		},
	};
}

const runtimes: AppRuntime[] = [];
afterEach(() => {
	for (const runtime of runtimes.splice(0)) runtime.dispose();
	vi.restoreAllMocks();
});

describe('engine preview adapter', () => {
	it('reattaches to live progress and shows the accepted artwork', async () => {
		const engine = createFakeEngine();
		engine.change((state) => {
			state.output.previewRun = preview();
			state.output.submissionInProgress = true;
		});
		const first = createAppRuntime({ engine });
		runtimes.push(first);
		await first.initialize();
		render(() => (
			<AppRuntimeProvider runtime={first}>
				<StatusPanelView />
				<PreviewAudioControls />
			</AppRuntimeProvider>
		));
		expect(screen.getByRole('button', { name: 'Start Processing' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Preview Audio' })).toBeDisabled();
		expect(screen.getByRole('button', { name: /^Cancel$/ })).toBeEnabled();

		await vi.waitFor(() =>
			expect(first.processing.status().coverArtSrc).toBe(
				coverSrc({ kind: 'preview', runId: 'preview-1' }),
			),
		);
		first.dispose();
		const replacement = createAppRuntime({ engine });
		runtimes.push(replacement);
		await replacement.initialize();
		expect(replacement.processing.status().progressPercentage).toBe(42);
		expect(replacement.processing.isProcessing()).toBe(true);
		replacement.processing.cancelAll();
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'cancelPreview',
				runId: 'preview-1',
				childJobId: null,
			}),
		);
	});

	it('opens only a path the engine grants, once, and never opens a cancelled run', async () => {
		const open = vi.spyOn(tauriClient, 'openPath').mockResolvedValue(undefined);
		const engine = createFakeEngine();
		engine.respond = (intent) =>
			intent.kind === 'takePreviewOutput'
				? { kind: 'previewOutput', path: '/tmp/Book.preview.m4b' }
				: undefined;
		const app = createAppRuntime({ engine });
		runtimes.push(app);
		await app.initialize();
		const complete = preview();
		complete.operation.status = 'completed';
		complete.openReady = true;
		complete.artworkReady = false;
		engine.change((state) => {
			state.output.previewRun = complete;
		});
		await vi.waitFor(() => expect(open).toHaveBeenCalledWith('/tmp/Book.preview.m4b'));
		engine.change((state) => {
			state.output.previewRun = { ...complete };
		});
		expect(open).toHaveBeenCalledTimes(1);
		const cancelled = preview();
		cancelled.operation.operationId = 'preview-2';
		cancelled.operation.status = 'cancelled';
		cancelled.operation.cancelRequested = true;
		cancelled.artworkReady = false;
		engine.change((state) => {
			state.output.previewRun = cancelled;
			state.output.submissionInProgress = false;
		});
		expect(app.processing.isProcessing()).toBe(false);
		expect(open).toHaveBeenCalledTimes(1);
		expect(
			engine.sessionIntents.filter((intent) => intent.kind === 'takePreviewOutput'),
		).toHaveLength(1);
	});
	it('finishes an accepted open when its frontend is disposed while the reply is pending', async () => {
		const open = vi.spyOn(tauriClient, 'openPath').mockResolvedValue(undefined);
		const engine = createFakeEngine();
		let finish!: (reply: SessionOutcome) => void;
		engine.respond = (intent) =>
			intent.kind === 'takePreviewOutput'
				? new Promise<SessionOutcome>((resolve) => {
						finish = resolve;
					})
				: undefined;
		const app = createAppRuntime({ engine });
		runtimes.push(app);
		await app.initialize();
		const complete = preview();
		complete.openReady = true;
		complete.artworkReady = false;
		complete.operation.status = 'completed';
		engine.change((state) => {
			state.output.previewRun = complete;
		});
		await vi.waitFor(() => expect(finish).toBeDefined());
		app.dispose();
		finish({ kind: 'previewOutput', path: '/tmp/accepted.preview.m4b' });
		await vi.waitFor(() => expect(open).toHaveBeenCalledWith('/tmp/accepted.preview.m4b'));
	});
});
