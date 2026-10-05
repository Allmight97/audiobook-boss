import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, SupportedAudioImportMetadata } from '../../types/audio';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import type { InputCapability, NativeDropPayload } from '../../lib/tauri/capabilities/input';
import { App } from '../App';

const metadata: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
	formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
	supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
};

const listeners: {
	drop?: (payload: NativeDropPayload) => void;
} = {};

function analyzedFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
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

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => []),
		openDirectory: vi.fn(async () => null),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		listenDragDrop: vi.fn(async (handler) => {
			listeners.drop = handler;
			return () => {
				listeners.drop = undefined;
			};
		}),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		...overrides,
	};
}

let engine: FakeEngine;

/** A runtime whose engine analyzes any import as `files`, or one file per path. */
function runtimeFor(
	options: { files?: AudioFile[]; input?: Partial<InputCapability> } = {},
): AppRuntime {
	engine = createFakeEngine();
	const analyze = options.files
		? () => options.files ?? []
		: (paths: readonly string[]) => paths.map((path) => analyzedFile(path));
	engine.analyze = vi.fn(analyze);
	return createAppRuntime({ input: fakeInput(options.input), engine });
}

/** Renders the app and waits for the engine's first snapshot. */
async function renderApp(runtime: AppRuntime) {
	const rendered = render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	));
	await screen.findByTestId('left-column');
	return rendered;
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
		runtime = runtimeFor({
			input: { openFiles: vi.fn(async () => ['/tmp/file1.mp3']) },
			files: [
				analyzedFile('/tmp/file1.mp3', {
					bitrate: 125_589,
					sampleRate: 22_050,
					channels: 2,
					codecLabel: 'MP3',
				}),
			],
		});
		await renderApp(runtime);

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
		runtime = runtimeFor({ files });
		await renderApp(runtime);
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		// The engine holds different choices for the first two titles.
		engine.change((state) => {
			const first = state.audio.titles[files[0]!.path]!;
			const second = state.audio.titles[files[1]!.path]!;
			for (const [title, channels, profile] of [
				[first, 'mono', 'aac_lc'],
				[second, 'stereo', 'he_aac_v1'],
			] as const) {
				title.choice = {
					...title.choice,
					intent: 'encode',
					encoder: 'faac',
					channels,
					faacProfile: profile,
				};
				title.facts = {
					...title.facts,
					effectiveEncoder: 'faac',
					faacProfiles: ['auto', 'aac_lc', 'he_aac_v1'],
				};
			}
			// The engine's answer for the two selected titles.
			state.audio.selection = {
				titleIds: [files[0]!.path, files[1]!.path],
				choice: first.choice,
				facts: first.facts,
				mixed: ['faacProfile', 'channels'],
			};
		});
		await runtime.input.selectFile({ index: 0, modifiers: { multi: false, range: false } });
		await runtime.input.selectFile({ index: 1, modifiers: { multi: true, range: false } });
		await user.click(screen.getByRole('button', { name: 'Audio settings · 2 titles' }));
		const editor = screen.getByRole('dialog', { name: 'Audio settings for 2 selected titles' });
		await user.click(within(editor).getByText(/Encoding settings/));
		expect(within(editor).getByLabelText('Channels')).toHaveValue('');
		expect(within(editor).getByLabelText('Profile')).toHaveValue('');

		await user.selectOptions(within(editor).getByLabelText('Channels'), 'mono');
		await waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setTitleAudio',
				titleIds: [files[0]!.path, files[1]!.path],
				edit: { field: 'channels', value: 'mono' },
			}),
		);
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog', { name: /Audio settings for/ })).not.toBeInTheDocument();

		await user.click(screen.getByRole('button', { name: 'Audio plan for other.m4b' }));
		const titleEditor = screen.getByRole('dialog', { name: 'Audio plan' });
		await user.selectOptions(within(titleEditor).getByLabelText('Audio handling'), 'encode');
		await waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setTitleAudio',
				titleIds: [files[2]!.path],
				edit: { field: 'intent', value: 'encode' },
			}),
		);
		expect(runtime.input.view().selectedIndices).toEqual([0, 1]);
		await user.keyboard('{Escape}');
		await user.click(screen.getByRole('button', { name: 'Audio settings · 2 titles' }));
		await runtime.input.selectFile({ index: 2, modifiers: { multi: false, range: false } });
		expect(screen.queryByRole('dialog', { name: /Audio settings for/ })).not.toBeInTheDocument();
	});

	it('shows the engine plan, its failure, and the estimate in the audio plan popover', async () => {
		const user = userEvent.setup();
		const file = analyzedFile('/books/estimate.m4b');
		runtime = runtimeFor({ files: [file] });
		await renderApp(runtime);
		await runtime.input.importIntent({ type: 'importPaths', paths: [file.path] });
		expect(screen.queryByTitle('Estimated output size')).not.toBeInTheDocument();

		const indicator = screen.getByRole('button', { name: 'Audio plan for estimate.m4b' });
		await user.click(indicator);
		const popup = screen.getByRole('dialog', { name: 'Audio plan' });
		expect(popup).toHaveTextContent('Checking source audio…');

		engine.seedTitleAudio(file.path, titleAudioRequest(), {
			plan: {
				kind: 'resolved',
				plan: {
					format: 'm4b',
					handling: 'preserve',
					settings: null,
					sampleRate: 44100,
					channels: 6,
					sourceCodec: 'AAC',
					reason: null,
				},
			},
			estimate: { kind: 'bytes', bytes: 1000 },
		});
		await waitFor(() => expect(popup).toHaveTextContent('6 channels'));
		expect(screen.getByTitle('Estimated output size')).toHaveTextContent('1000.0 B');

		engine.seedTitleAudio(file.path, titleAudioRequest(), {
			plan: { kind: 'failed', message: 'Sources cannot be joined', field: null },
			estimate: null,
		});
		await waitFor(() => expect(popup).toHaveTextContent('Sources cannot be joined'));
		expect(screen.queryByTitle('Estimated output size')).not.toBeInTheDocument();
	});

	it('shows an invalid source in the grouped title status', async () => {
		const files = [
			analyzedFile('/books/valid.m4b'),
			analyzedFile('/books/broken.m4b', { isValid: false, error: 'Broken audio source' }),
		];
		runtime = runtimeFor({ files });
		await renderApp(runtime);
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		engine.seedGroup(files);
		await waitFor(() => {
			const row = screen.getByRole('option', { name: 'valid.m4b' });
			expect(row).toHaveClass('invalid');
			expect(row).toHaveTextContent('Broken audio source');
		});
	});

	it('keeps a grouped title’s PDF chip when only a later source has a companion', async () => {
		const files = [analyzedFile('/books/part1.m4b'), analyzedFile('/books/part2.m4b')];
		runtime = runtimeFor({ files });
		await renderApp(runtime);
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		engine.seedGroup(files);
		engine.change((state) => {
			state.titles.companions = { [files[1]!.inputId!]: ['Guide.pdf'] };
		});
		await waitFor(() => {
			const row = screen.getByRole('option', { name: 'part1.m4b' });
			expect(within(row).getByText('PDF')).toBeVisible();
		});
	});

	it('edits one stack through its audio popover', async () => {
		const user = userEvent.setup();
		const files = ['part1.mp3', 'part2.mp3', 'other.m4b'].map((name) =>
			analyzedFile(`/books/${name}`, { sampleRate: 44100, channels: 1, codecLabel: 'MP3' }),
		);
		runtime = runtimeFor({ files });
		await renderApp(runtime);
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		engine.seedTitleAudio(files[0]!.path, titleAudioRequest({ format: 'mp3', settings: null }));
		// The engine grouped the first two, whose audio disagrees.
		engine.seedGroup([files[0]!, files[1]!], { choiceRequired: true });
		engine.respond = (intent) => {
			if (intent.kind !== 'applyDefaultAudio') return undefined;
			engine.change((state) => {
				state.titles.audioChoiceRequired = [];
			});
			return { kind: 'applied' };
		};
		await waitFor(() => expect(runtime!.input.audioChoiceRequired(files[0]!)).toBe(true));
		const indicator = screen.getByRole('button', { name: 'Audio plan for part1.mp3' });
		await user.hover(indicator);
		const popup = await screen.findByRole('dialog', { name: 'Audio plan' });
		expect(popup).toHaveTextContent('Choose this title’s audio');
		expect(popup.parentElement).toBe(document.body);
		await user.click(indicator);
		await user.click(within(popup).getByRole('button', { name: 'Apply App Settings' }));
		await waitFor(() => expect(runtime!.input.audioChoiceRequired(files[0]!)).toBe(false));
		expect(engine.sessionIntents).toContainEqual({
			kind: 'applyDefaultAudio',
			titleIds: [files[0]!.path],
		});
		expect(within(popup).queryByRole('option', { name: /App default/ })).not.toBeInTheDocument();
		await user.selectOptions(within(popup).getByLabelText('Output'), 'm4aOpus');
		await waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setTitleAudio',
				titleIds: [files[0]!.path],
				edit: { field: 'format', value: 'm4aOpus' },
			}),
		);
		await user.keyboard('{Escape}');
		// Browser focus/hover can arrive after the popover unmounts.
		fireEvent.focus(indicator);
		fireEvent.mouseEnter(indicator);
		expect(screen.queryByRole('dialog', { name: 'Audio plan' })).not.toBeInTheDocument();
	});

	it('handles keyboard actions only from the focused listbox', async () => {
		runtime = runtimeFor({
			files: [analyzedFile('/books/alpha.m4b'), analyzedFile('/books/bravo.m4b')],
		});
		await renderApp(runtime);
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
		runtime = runtimeFor();
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

		await renderApp(runtime);
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
			expect(engine.sessionIntents).toContainEqual({
				kind: 'loadCoverFromDrop',
				paths: ['/tmp/image.png'],
			});
		});
		expect(engine.analyze).not.toHaveBeenCalled();

		listeners.drop?.({ position: { x: 200, y: 200 }, paths: ['/tmp/file1.wav'] });
		await waitFor(() => {
			expect(engine.analyze).toHaveBeenCalledWith(['/tmp/file1.wav']);
		});
	});

	it('blocks import while order is locked and surfaces the lock banner', async () => {
		runtime = runtimeFor();
		await renderApp(runtime);
		engine.change((state) => {
			state.titles.orderLocked = true;
		});
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
		});
		void runtime.input.importIntent({ type: 'importPaths', paths: ['/tmp/file1.mp3'] });
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
			expect(screen.getByText(/Wait for completion to add files/)).toBeInTheDocument();
		});
		expect(engine.analyze).not.toHaveBeenCalled();
	});
});
