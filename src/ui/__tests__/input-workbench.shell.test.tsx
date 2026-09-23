import { titleAudioRequest } from '../../test/fixtures/titleAudio';
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

	it('imports audio and enables grouping only for multiple selected titles', async () => {
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
		expect(screen.getByRole('button', { name: 'Group as one title' })).toBeDisabled();
		expect(runtime.input.view().selectedIndices).toEqual([0]);
	});

	it('edits only selected titles through one mixed-value audio editor', async () => {
		const user = userEvent.setup();
		const files = ['first.m4b', 'second.m4b', 'other.m4b'].map((name) =>
			analyzedFile(`/books/${name}`),
		);
		runtime = createAppRuntime({
			input: fakeInput({ analyzeAudioFiles: vi.fn(async () => analyzedList(files)) }),
		});
		renderApp(runtime);
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await waitFor(() => expect(runtime!.encoding.view().flavorOptions.length).toBeGreaterThan(1));
		runtime.encoding.selectTitle(files[0]!, 'channels', 'mono');
		runtime.encoding.selectTitle(files[1]!, 'channels', 'stereo');
		const untouched = runtime.encoding.audioRequest(files[2]!);
		await runtime.input.selectFile({ index: 0, modifiers: { multi: false, range: false } });
		await runtime.input.selectFile({ index: 1, modifiers: { multi: true, range: false } });
		await user.click(screen.getByRole('button', { name: 'Audio settings · 2 titles' }));
		const editor = screen.getByRole('dialog', { name: 'Audio settings for 2 selected titles' });
		await user.click(within(editor).getByText(/Encoding settings/));
		expect(within(editor).getByLabelText('Channels')).toHaveValue('');
		await user.selectOptions(within(editor).getByLabelText('Channels'), 'mono');
		expect(runtime.encoding.audioRequest(files[0]!).settings?.channels).toBe('mono');
		expect(runtime.encoding.audioRequest(files[1]!).settings?.channels).toBe('mono');
		expect(runtime.encoding.audioRequest(files[2]!)).toEqual(untouched);
		await user.selectOptions(within(editor).getByLabelText('Audio handling'), 'preserve');
		expect(runtime.encoding.audioRequest(files[0]!).intent).toBe('preserve');
		expect(runtime.encoding.audioRequest(files[1]!).intent).toBe('preserve');
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog', { name: /Audio settings for/ })).not.toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: 'Audio plan for other.m4b' }));
		const titleEditor = screen.getByRole('dialog', { name: 'Audio plan' });
		await user.click(within(titleEditor).getByText('Encoding settings', { exact: true }));
		expect(runtime.input.view().selectedIndices).toEqual([0, 1]);
		expect(titleEditor).toBeInTheDocument();
		await user.keyboard('{Escape}');
		await user.click(screen.getByRole('button', { name: 'Audio settings · 2 titles' }));
		await runtime.input.selectFile({ index: 2, modifiers: { multi: false, range: false } });
		expect(screen.queryByRole('dialog', { name: /Audio settings for/ })).not.toBeInTheDocument();
	});

	it('keeps a grouped title’s PDF chip when only a later source has a companion', async () => {
		const files = [analyzedFile('/books/part1.m4b'), analyzedFile('/books/part2.m4b')];
		runtime = createAppRuntime({
			input: fakeInput({ analyzeAudioFiles: vi.fn(async () => analyzedList(files)) }),
		});
		vi.spyOn(runtime.remoteSource, 'hasCompanions').mockImplementation(
			(inputId) => inputId === files[1]!.inputId,
		);
		renderApp(runtime);
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await runtime.input.selectAll();
		await runtime.input.groupSelected();
		const row = screen.getByRole('option', { name: 'part1.m4b' });
		expect(within(row).getByText('PDF')).toBeVisible();
	});

	it.each([false, true])(
		'validates only submitted title sources (grouped: %s)',
		async (grouped) => {
			const files = [
				analyzedFile('/books/valid.m4b', {
					preservation: { canPreserve: true },
				}),
				analyzedFile('/books/invalid.m4b', { isValid: false }),
			];
			const preflight = vi.spyOn(tauriClient, 'preflightProcessingPlan').mockResolvedValue({
				jobType: 'batch',
				collisionPolicy: 'fail',
				audioPlans: [],
				planSignature: 'valid-titles',
				outputs: [],
			});
			const submit = vi.spyOn(tauriClient, 'submitProcessingOperation');
			runtime = createAppRuntime({
				input: fakeInput({ analyzeAudioFiles: vi.fn(async () => analyzedList(files)) }),
			});
			await runtime.input.importIntent({
				type: 'importPaths',
				paths: files.map((file) => file.path),
			});
			runtime.input.setAudioRequest(files[0]!, titleAudioRequest({ intent: 'auto' }));
			if (grouped) {
				await runtime.input.selectAll();
				await runtime.input.groupSelected();
				runtime.input.setAudioRequest(files[0]!, titleAudioRequest({ intent: 'encode' }));
			}
			runtime.output.applyDefaults({
				outputDirectory: '/library',
				outputNaming: { preset: 'absDefault', includeYear: false },
			});
			try {
				await runtime.processing.start();
				if (grouped) {
					expect(preflight).not.toHaveBeenCalled();
					expect(submit).not.toHaveBeenCalled();
					expect(runtime.processing.status().stepText).toContain('invalid source');
				} else {
					expect(submit).toHaveBeenCalledWith(
						expect.objectContaining({
							payload: expect.objectContaining({ inputFiles: ['/books/valid.m4b'] }),
						}),
					);
				}
			} finally {
				preflight.mockRestore();
				submit.mockRestore();
			}
		},
	);

	it('edits one stack through its audio popover and submits it beside an independent title', async () => {
		const user = userEvent.setup();
		const files = ['part1.mp3', 'part2.mp3', 'other.m4b'].map((name) =>
			analyzedFile(`/books/${name}`, { sampleRate: 44100, channels: 1, codecLabel: 'MP3' }),
		);
		const preview = vi.spyOn(tauriClient, 'previewTitleAudio').mockResolvedValue({
			format: 'mp3',
			handling: 'preserve',
			settings: null,
			sampleRate: 44100,
			channels: 1,
			sourceCodec: 'MP3',
			reason: null,
		});
		const preflight = vi.spyOn(tauriClient, 'preflightProcessingPlan').mockResolvedValue({
			jobType: 'batch',
			collisionPolicy: 'fail',
			planSignature: 'stack-review',
			audioPlans: [],
			outputs: [],
		});
		const submit = vi.spyOn(tauriClient, 'submitProcessingOperation');
		runtime = createAppRuntime({
			input: fakeInput({ analyzeAudioFiles: vi.fn(async () => analyzedList(files)) }),
		});
		renderApp(runtime);
		try {
			await runtime.input.importIntent({
				type: 'importPaths',
				paths: files.map((file) => file.path),
			});
			await waitFor(() => expect(runtime!.encoding.view().flavorOptions.length).toBeGreaterThan(1));
			runtime.encoding.select('quality', '4');
			runtime.input.setAudioRequest(
				files[0]!,
				titleAudioRequest({ format: 'mp3', settings: null }),
			);
			await runtime.input.selectFile({ index: 0, modifiers: { multi: false, range: false } });
			await runtime.input.selectFile({ index: 1, modifiers: { multi: true, range: false } });
			await runtime.input.groupSelected();
			runtime.input.reorderSources(files[0]!, 0, 1);
			expect(runtime.input.audioChoiceRequired(files[0]!)).toBe(true);
			const indicator = screen.getByRole('button', { name: 'Audio plan for part1.mp3' });
			await user.hover(indicator);
			const popup = await screen.findByRole('dialog', { name: 'Audio plan' });
			expect(popup).toHaveTextContent('Choose this title’s audio');
			expect(popup.parentElement).toBe(document.body);
			await user.click(indicator);
			await user.click(within(popup).getByRole('button', { name: 'Use defaults' }));
			expect(runtime.input.audioChoiceRequired(files[0]!)).toBe(false);
			expect(runtime.encoding.audioRequest(files[0]!).format).toBe('m4b');
			await user.selectOptions(within(popup).getByLabelText('Output'), 'm4aOpus');
			expect(runtime.input.audioChoiceRequired(files[0]!)).toBe(false);
			await user.selectOptions(within(popup).getByLabelText('Audio handling'), 'encode');
			await user.keyboard('{Escape}');
			// Browser focus/hover can arrive after the popover unmounts.
			fireEvent.focus(indicator);
			fireEvent.mouseEnter(indicator);
			expect(screen.queryByRole('dialog', { name: 'Audio plan' })).not.toBeInTheDocument();
			expect(runtime.encoding.audioRequest(files[0]!).format).toBe('m4aOpus');
			expect(runtime.encoding.audioRequest(files[2]!).format).toBe('m4b');
			expect(runtime.encoding.audioRequest(files[2]!).settings?.bitrateMode).toEqual({
				mode: 'vbr',
				value: 3,
			});
			runtime.output.applyDefaults({
				outputDirectory: '/library',
				outputNaming: { preset: 'absDefault', includeYear: false },
			});
			await runtime.processing.start();
			expect(submit).toHaveBeenCalledWith(
				expect.objectContaining({
					payload: expect.objectContaining({
						inputFiles: [files[0]!.path, files[2]!.path],
						titleSources: {
							[files[0]!.path]: [
								{ path: files[1]!.path, inputId: files[1]!.inputId },
								{ path: files[0]!.path, inputId: files[0]!.inputId },
							],
						},
						audioRequests: [
							expect.objectContaining({
								format: 'm4aOpus',
								intent: 'encode',
								settings: expect.objectContaining({ encoderType: 'opus' }),
							}),
							expect.objectContaining({
								format: 'm4b',
								settings: expect.objectContaining({ bitrateMode: { mode: 'vbr', value: 3 } }),
							}),
						],
					}),
				}),
			);
		} finally {
			preview.mockRestore();
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
