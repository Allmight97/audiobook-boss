import { cleanup, fireEvent, render, screen, waitFor, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createAppRuntime, type AppRuntime } from '../../app/runtime';
import type { InputCapability, NativeDropPayload } from '../../lib/tauri/capabilities/input';
import { App } from '../App';
import { tauriClient } from '../../lib/tauri/client';

const metadata: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
	formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
	supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
};

const listeners: {
	drop?: (payload: NativeDropPayload) => void;
} = {};

function analyzedFile(
	path: string,
	overrides: Record<string, unknown> = {},
): FileListInfo['files'][number] {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 1000,
		format: 'mp3',
		inputId: path,
		...overrides,
	};
}

function analyzedList(files: FileListInfo['files']): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length * 1000,
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
	};
}

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => []),
		openDirectory: vi.fn(async () => null),
		discoverAudioImportPaths: vi.fn(async (paths: ReadonlyArray<string>) =>
			paths.filter((path) => !path.endsWith('.txt') && !path.endsWith('.png')),
		),
		analyzeAudioFiles: vi.fn(async (paths: ReadonlyArray<string>) =>
			analyzedList(paths.map((path) => analyzedFile(path))),
		),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		takeOpenedAudioFiles: vi.fn(async () => []),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async (handler) => {
			listeners.drop = handler;
			return () => {
				listeners.drop = undefined;
			};
		}),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
		...overrides,
	};
}

function renderApp(runtime: AppRuntime) {
	return render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	));
}

describe('Solid input workbench', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		listeners.drop = undefined;
		document.getElementById('cover-art-area')?.remove();
	});

	it('imports audio and preserves the selected list when switching to merge', async () => {
		const user = userEvent.setup();
		const input = fakeInput({
			openFiles: vi.fn(async () => ['/tmp/file1.mp3']),
			analyzeAudioFiles: vi.fn(async () =>
				analyzedList([
					analyzedFile('/tmp/file1.mp3', {
						bitrate: 125_589,
						sampleRate: 22_050,
						channels: 2,
						codecLabel: 'MP3',
					}),
				]),
			),
		});
		runtime = createAppRuntime({ input });
		renderApp(runtime);

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		const row = await screen.findByRole('option', { name: 'file1.mp3' });
		expect(within(row).getByText('125.6 kbps · 22.05 kHz · Stereo · MP3')).toBeVisible();
		expect(runtime.input.view().files).toHaveLength(1);
		expect(runtime.input.jobType()).toBe('batch');

		await user.click(screen.getByLabelText('Merge files into one audiobook'));
		expect(runtime.input.jobType()).toBe('merge');
		expect(runtime.input.view().files).toHaveLength(1);
		expect(runtime.input.view().selectedIndices).toEqual([0]);
	});

	it('exports five tagged books with three explicitly preserved and two using the selected encoder', async () => {
		const user = userEvent.setup();
		const books = ['Prey 1', 'Prey 2', 'Prey 3', 'Large', 'Standard'].map((title, index) =>
			analyzedFile(`/books/${title}.m4b`, {
				tagTitle: title,
				bitrate: index < 3 ? 64_000 : 128_000,
				sampleRate: index < 3 ? 22_050 : 44_100,
				channels: 2,
				codecLabel: 'AAC-LC',
				preservation: { canPreserve: true, recommended: index < 3 },
			}),
		);
		const readMetadata = vi
			.spyOn(tauriClient, 'readAudioMetadata')
			.mockResolvedValue({ title: 'Original' });
		const preflight = vi.spyOn(tauriClient, 'preflightProcessingPlan').mockResolvedValue({
			jobType: 'batch',
			collisionPolicy: 'fail',
			planSignature: 'mixed-review',
			outputs: books.map((file, inputIndex) => ({
				inputIndex,
				inputPath: file.path,
				kind: 'final',
				requestedPath: `/library/${inputIndex}.m4b`,
				resolvedPath: `/library/${inputIndex}.m4b`,
				action: 'write',
			})),
		});
		const submit = vi.spyOn(tauriClient, 'submitProcessingOperation');
		runtime = createAppRuntime({
			input: fakeInput({ analyzeAudioFiles: vi.fn(async () => analyzedList(books)) }),
		});
		renderApp(runtime);
		try {
			await runtime.input.importIntent({
				type: 'importPaths',
				paths: books.map((book) => book.path),
			});
			const rows = await within(screen.getByRole('listbox', { name: 'Audio files' })).findAllByRole(
				'option',
			);
			const selected = [...runtime.input.view().selectedIndices];
			expect(screen.getAllByRole('button', { name: /Why keep original audio/ })).toHaveLength(3);
			expect(within(rows[0]!).getByText('64 kbps · 22.05 kHz · Stereo · AAC-LC')).toBeVisible();
			const info = within(rows[0]!).getByRole('button', { name: /Why keep original audio/ });
			await user.hover(info);
			expect(within(rows[0]!).getByRole('note')).toHaveTextContent(
				'This audiobook may not need re-encoding',
			);
			await fireEvent.keyDown(document.body, { key: 'Escape' });
			expect(within(rows[0]!).queryByRole('note')).not.toBeInTheDocument();
			await user.click(info);
			await user.unhover(info);
			expect(within(rows[0]!).getByRole('note')).toBeVisible();
			await user.click(
				within(rows[0]!).getByRole('button', { name: 'Keep original audio for Prey 1' }),
			);
			await user.keyboard('{Escape}');
			expect(within(rows[0]!).queryByRole('note')).not.toBeInTheDocument();
			for (const row of rows.slice(1, 3))
				await user.click(within(row).getByText('Keep original audio'));
			expect(runtime.input.view().selectedIndices).toEqual(selected);
			expect(
				within(rows[3]!).getByRole('checkbox', { name: 'Keep original audio for Large' }),
			).not.toBeChecked();
			expect(runtime.input.view().files.map((file) => runtime!.input.audioHandling(file))).toEqual([
				'preserve',
				'preserve',
				'preserve',
				'encode',
				'encode',
			]);
			await waitFor(() => expect(runtime!.encoding.view().flavorOptions.length).toBeGreaterThan(1));
			runtime.encoding.select('encoder', 'faac_he_aac');
			runtime.encoding.select('sampleRate', '44100');
			runtime.output.applyDefaults({
				outputDirectory: '/library',
				outputNaming: { preset: 'absDefault', includeYear: false },
			});
			const metadataIntent = Object.fromEntries(
				books.map((file, index) => [
					file.path,
					{ title: { op: 'set' as const, value: `Library ${index + 1}` } },
				]),
			);
			for (const file of books) runtime.metadata.stageIntent(file.path, metadataIntent[file.path]!);
			await user.click(document.getElementById('process-button') as HTMLElement);
			await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
			expect(submit).toHaveBeenCalledWith(
				expect.objectContaining({
					payload: expect.objectContaining({
						audioHandling: ['preserve', 'preserve', 'preserve', 'encode', 'encode'],
						inputFiles: books.map((file) => file.path),
						outputDir: '/library',
						settings: expect.objectContaining({ encoderType: 'faac_he_aac' }),
						sampleRate: { explicit: 44100 },
						outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
					}),
					metadataIntent,
				}),
			);
		} finally {
			readMetadata.mockRestore();
			preflight.mockRestore();
			submit.mockRestore();
		}
	});

	it('handles keyboard actions only from the focused listbox', async () => {
		const input = fakeInput({
			analyzeAudioFiles: vi.fn(async () =>
				analyzedList([analyzedFile('/books/alpha.m4b'), analyzedFile('/books/bravo.m4b')]),
			),
		});
		runtime = createAppRuntime({ input });
		renderApp(runtime);
		void runtime.input.importIntent({
			type: 'importPaths',
			paths: ['/books/alpha.m4b', '/books/bravo.m4b'],
		});
		const listbox = await screen.findByRole('listbox', { name: 'Audio files' });
		await waitFor(() => {
			expect(within(listbox).getAllByRole('option')).toHaveLength(2);
		});
		await fireEvent.keyDown(listbox, { key: 'ArrowDown' });
		await waitFor(() => {
			expect(runtime?.input.view().selectedIndices).toEqual([0]);
		});

		await fireEvent.keyDown(document.body, { key: 'ArrowDown' });
		expect(runtime.input.view().selectedIndices).toEqual([0]);

		await fireEvent.keyDown(listbox, { key: 'a', ctrlKey: true });
		await waitFor(() => {
			expect(runtime?.input.view().selectedIndices).toEqual([0, 1]);
		});
	});

	it('routes cover-art drops away from import and imports file-area drops', async () => {
		const input = fakeInput();
		runtime = createAppRuntime({ input });
		const cover = document.createElement('div');
		cover.id = 'cover-art-area';
		document.body.appendChild(cover);
		cover.getBoundingClientRect = () =>
			({
				left: 0,
				right: 100,
				top: 0,
				bottom: 100,
				width: 100,
				height: 100,
				x: 0,
				y: 0,
				toJSON: () => ({}),
			}) as DOMRect;

		renderApp(runtime);
		await waitFor(() => {
			expect(listeners.drop).toBeTypeOf('function');
		});

		const container = document.querySelector('.file-management-container') as HTMLElement;
		container.getBoundingClientRect = () =>
			({
				left: 150,
				right: 400,
				top: 150,
				bottom: 350,
				width: 250,
				height: 200,
				x: 150,
				y: 150,
				toJSON: () => ({}),
			}) as DOMRect;

		listeners.drop?.({ position: { x: 50, y: 50 }, paths: ['/tmp/image.png'] });
		await waitFor(() => {
			expect(input.analyzeAudioFiles).not.toHaveBeenCalled();
		});

		listeners.drop?.({ position: { x: 200, y: 200 }, paths: ['/tmp/file1.wav'] });
		await waitFor(() => {
			expect(input.analyzeAudioFiles).toHaveBeenCalledWith(['/tmp/file1.wav']);
		});
	});

	it('blocks import while order is locked and surfaces the lock banner', async () => {
		const input = fakeInput();
		runtime = createAppRuntime({ input });
		renderApp(runtime);
		runtime.input.setOrderLocked(true);
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
		});
		void runtime.input.importIntent({ type: 'importPaths', paths: ['/tmp/file1.mp3'] });
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
			expect(screen.getByText(/Wait for completion to add files/)).toBeInTheDocument();
		});
		expect(input.analyzeAudioFiles).not.toHaveBeenCalled();
	});
});
