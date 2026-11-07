import { titleAudioRequest } from '../../test/fixtures/titleAudio';
import { fireEvent, screen, waitFor, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, onTestFinished, vi } from 'vitest';
import {
	type AudioFile,
	SUPPORTED_AUDIO_IMPORT_EXTENSIONS,
	SUPPORTED_AUDIO_IMPORT_SUPPORT_TEXT,
	type SessionOutcome,
} from '../../lib/generated/tauri';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { renderApp } from '../../test/fixtures/renderApp';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { tauriClient } from '../../lib/tauri/client';

const REMOVED_NOTICE =
	'Exported. Its download was removed to save space; acquire it again to export it again.';

let engine: FakeEngine;

async function start(options: { files?: AudioFile[]; input?: Partial<InputCapability> } = {}) {
	engine = createFakeEngine();
	const files = options.files;
	engine.analyze = vi.fn(
		files
			? () => files
			: (paths: readonly string[]) => paths.map((path) => audioFile(path, { status: 'ready' })),
	);
	return await renderApp({ engine, input: options.input });
}

function selects(selectedIndices: number[], selectedAnchor: number): SessionOutcome {
	engine.change((state) => {
		state.selection = { ...state.selection, selectedIndices, selectedAnchor };
	});
	return { kind: 'applied' };
}

describe('Solid input workbench', () => {
	it('imports audio and enables grouping only for multiple selected titles', async () => {
		const user = userEvent.setup();
		const { runtime } = await start({
			input: { openFiles: vi.fn(async () => ['/tmp/file1.mp3']) },
			files: [
				audioFile('/tmp/file1.mp3', {
					status: 'ready',
					bitrate: 125_589,
					sampleRate: 22_050,
					channels: 2,
					codecLabel: 'MP3',
				}),
			],
		});
		engine.respond = (intent) => {
			if (intent.kind !== 'import') return undefined;
			engine.loadTitles(engine.analyze(intent.paths), [0]);
			return { kind: 'applied' };
		};

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		const row = await screen.findByRole('option', { name: 'file1.mp3' });
		expect(within(row).getByText('125.6 kbps · 22.05 kHz · Stereo · MP3')).toBeVisible();
		expect(runtime.input.view().files).toHaveLength(1);
		expect(screen.getByRole('button', { name: 'Group as one title' })).toBeDisabled();
		expect(runtime.input.view().selectedIndices).toEqual([0]);
	});

	it('opens the picker with the engine import formats and shows the engine support text', async () => {
		const user = userEvent.setup();
		const openFiles = vi.fn(async () => []);
		const { runtime } = await start({ input: { openFiles } });

		expect(runtime.input.view().supportText).toBe(SUPPORTED_AUDIO_IMPORT_SUPPORT_TEXT);
		await user.click(screen.getByRole('button', { name: 'Add audio files' }));

		expect(openFiles).toHaveBeenCalledWith({
			filters: [{ name: 'Audio Files', extensions: [...SUPPORTED_AUDIO_IMPORT_EXTENSIONS] }],
		});
	});

	it('edits only selected titles through one mixed-value audio editor', async () => {
		const user = userEvent.setup();
		const files = ['first.m4b', 'second.m4b', 'other.m4b'].map((name) =>
			audioFile(`/books/${name}`, { status: 'ready' }),
		);
		const { runtime } = await start({ files });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await waitFor(() => expect(runtime.input.view().files).toHaveLength(files.length));
		engine.change((state) => {
			const first = state.audio.titles[files[0].path];
			const second = state.audio.titles[files[1].path];
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
			state.audio.selection = {
				choice: first.choice,
				facts: first.facts,
				mixed: ['faacProfile', 'channels'],
				autoSampleRate: { outcome: { kind: 'unresolved' }, problem: null },
				autoChannels: { outcome: { kind: 'unresolved' }, problem: null },
			};
		});
		const selectionAnswers = [
			() => selects([0], 0),
			() => selects([0, 1], 1),
			() => selects([2], 2),
		];
		engine.respond = (intent) => {
			if (intent.kind !== 'selectFile') return undefined;
			return selectionAnswers.shift()?.();
		};
		await runtime.input.selectFile({
			index: 0,
			modifiers: { multi: false, range: false },
		});
		await runtime.input.selectFile({
			index: 1,
			modifiers: { multi: true, range: false },
		});
		await user.click(screen.getByRole('button', { name: 'Audio settings · 2 titles' }));
		const editor = screen.getByRole('dialog', {
			name: 'Audio settings for 2 selected titles',
		});
		await user.click(within(editor).getByText(/Encoding settings/));
		expect(within(editor).getByLabelText('Channels')).toHaveValue('');
		expect(within(editor).getByLabelText('Profile')).toHaveValue('');

		await user.selectOptions(within(editor).getByLabelText('Channels'), 'mono');
		await waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setTitleAudio',
				titleIds: [files[0].path, files[1].path],
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
				titleIds: [files[2].path],
				edit: { field: 'intent', value: 'encode' },
			}),
		);
		expect(runtime.input.view().selectedIndices).toEqual([0, 1]);
		await user.keyboard('{Escape}');
		await user.click(screen.getByRole('button', { name: 'Audio settings · 2 titles' }));
		await runtime.input.selectFile({
			index: 2,
			modifiers: { multi: false, range: false },
		});
		expect(screen.queryByRole('dialog', { name: /Audio settings for/ })).not.toBeInTheDocument();
	});

	it('shows the engine plan, its failure, and the estimate in the audio plan popover', async () => {
		const user = userEvent.setup();
		const file = audioFile('/books/estimate.m4b', { status: 'ready', sampleRate: 22050 });
		const { runtime } = await start({ files: [file] });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: [file.path],
		});
		const indicator = await screen.findByRole('button', {
			name: 'Audio plan for estimate.m4b',
		});
		expect(screen.queryByTitle('Estimated output size')).not.toBeInTheDocument();

		await user.click(indicator);
		const popup = screen.getByRole('dialog', { name: 'Audio plan' });
		expect(popup).toHaveTextContent('Checking source audio…');

		engine.seedTitleAudio(file.path, titleAudioRequest(), {
			plan: {
				kind: 'resolved',
				plan: {
					format: 'm4b',
					handling: 'preserve',
					sampleRate: 44100,
					channels: 6,
					sourceCodec: 'AAC',
					reason: null,
				},
			},
			estimate: { kind: 'bytes', bytes: 1000 },
		});
		await waitFor(() => expect(popup).toHaveTextContent('6 channels'));
		expect(popup).toHaveTextContent('Sample rate44.1 kHz');
		expect(popup).toHaveTextContent('Keeping original audio');
		expect(screen.getByTitle('Estimated output size')).toHaveTextContent('1000.0 B');

		engine.seedTitleAudio(file.path, titleAudioRequest(), {
			plan: {
				kind: 'failed',
				message: 'Sources cannot be joined',
				field: null,
			},
			estimate: null,
		});
		await waitFor(() => expect(popup).toHaveTextContent('Sources cannot be joined'));
		expect(screen.queryByTitle('Estimated output size')).not.toBeInTheDocument();
	});

	it('shows a grouped title the engine marks failed as failed, with its error', async () => {
		const files = [
			audioFile('/books/valid.m4b', { status: 'ready' }),
			audioFile('/books/broken.m4b', {
				status: 'failed',
				isValid: false,
				error: 'Broken audio source',
			}),
		];
		const { runtime } = await start({ files });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await waitFor(() => expect(runtime.input.view().files).toHaveLength(files.length));
		engine.seedGroup(files);
		engine.change((state) => {
			const anchor = state.titles.files.find((file) => file.path === files[0].path);
			if (anchor) Object.assign(anchor, { status: 'failed', error: 'Broken audio source' });
		});
		await waitFor(() => {
			const row = screen.getByRole('option', { name: 'valid.m4b' });
			expect(row).toHaveClass('invalid');
			expect(row).toHaveTextContent('Error: Broken audio source');
		});
	});

	it('shows an exported title whose download was removed as finished, not as an error', async () => {
		const removed = audioFile('/books/done.m4b', {
			status: 'removedFinished',
			isValid: false,
			sourceRemoved: true,
			error: REMOVED_NOTICE,
		});
		const { runtime } = await start({ files: [removed] });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: [removed.path],
		});
		const row = await screen.findByRole('option', { name: 'done.m4b' });
		engine.seedTitleAudio(removed.path, titleAudioRequest(), {
			plan: { kind: 'failed', message: 'Source is not valid', field: null },
			estimate: null,
		});
		await waitFor(() => expect(row).toHaveTextContent(REMOVED_NOTICE));
		expect(row.querySelector('.file-status')).toHaveClass('file-status-valid');
		expect(row.querySelector('.file-status')).toHaveTextContent('✓');
		expect(row).not.toHaveClass('invalid');
		expect(row).not.toHaveTextContent('Error:');
		expect(row).not.toHaveTextContent('Choose audio');
		expect(screen.queryByRole('button', { name: 'Audio plan for done.m4b' })).toBeNull();
		expect(screen.queryByTitle('Estimated output size')).toBeNull();
	});

	it('keeps a grouped title’s PDF chip when only a later source has a companion', async () => {
		const files = [
			audioFile('/books/part1.m4b', { status: 'ready' }),
			audioFile('/books/part2.m4b', { status: 'ready' }),
		];
		const { runtime } = await start({ files });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await waitFor(() => expect(runtime.input.view().files).toHaveLength(files.length));
		engine.seedGroup(files);
		engine.change((state) => {
			state.titles.companions = { [files[1].inputId]: ['Guide.pdf'] };
		});
		await waitFor(() => {
			const row = screen.getByRole('option', { name: 'part1.m4b' });
			expect(within(row).getByText('PDF')).toBeVisible();
		});
	});

	it("shows a grouped title's chapter total from the engine", async () => {
		const files = ['part1.mp3', 'part2.mp3'].map((name) =>
			audioFile(`/books/${name}`, { status: 'ready' }),
		);
		const { runtime } = await start({ files });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await waitFor(() => expect(runtime.input.view().files).toHaveLength(files.length));
		engine.seedGroup(files);
		engine.change((state) => {
			state.titles.titleTotals = {
				[files[0].inputId]: { chapterCount: 2, durationSeconds: 2, sizeBytes: 2000 },
			};
		});
		await waitFor(() => {
			const row = screen.getByRole('option', { name: 'part1.mp3' });
			expect(row).toHaveTextContent('2 chapters');
		});
	});

	it('edits one stack through its audio popover', async () => {
		const user = userEvent.setup();
		const files = ['part1.mp3', 'part2.mp3', 'other.m4b'].map((name) =>
			audioFile(`/books/${name}`, {
				status: 'ready',
				sampleRate: 44100,
				channels: 1,
				codecLabel: 'MP3',
			}),
		);
		const { runtime } = await start({ files });
		await runtime.input.importIntent({
			type: 'importPaths',
			paths: files.map((file) => file.path),
		});
		await waitFor(() => expect(runtime.input.view().files).toHaveLength(files.length));
		engine.seedTitleAudio(files[0].path, titleAudioRequest());
		engine.seedGroup([files[0], files[1]], { choiceRequired: true });
		engine.respond = (intent) => {
			if (intent.kind !== 'applyDefaultAudio') return undefined;
			engine.change((state) => {
				state.audio.titles[files[0].path].plan = { kind: 'pending' };
			});
			return { kind: 'applied' };
		};
		await waitFor(() => expect(runtime.encoding.plan(files[0]).kind).toBe('choiceRequired'));
		const indicator = screen.getByRole('button', {
			name: 'Audio plan for part1.mp3',
		});
		await user.hover(indicator);
		const popup = await screen.findByRole('dialog', { name: 'Audio plan' });
		expect(popup).toHaveTextContent('Choose this title’s audio');
		expect(popup.parentElement).toBe(document.body);
		await user.click(indicator);
		await user.click(within(popup).getByRole('button', { name: 'Apply App Settings' }));
		await waitFor(() => expect(runtime.encoding.plan(files[0]).kind).not.toBe('choiceRequired'));
		expect(engine.sessionIntents).toContainEqual({
			kind: 'applyDefaultAudio',
			titleIds: [files[0].path],
		});
		expect(within(popup).queryByRole('option', { name: /App default/ })).not.toBeInTheDocument();
		await user.selectOptions(within(popup).getByLabelText('Output'), 'm4aOpus');
		await waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setTitleAudio',
				titleIds: [files[0].path],
				edit: { field: 'format', value: 'm4aOpus' },
			}),
		);
		await user.keyboard('{Escape}');
		// Browser focus/hover can arrive after the popover unmounts.
		fireEvent.focus(indicator);
		fireEvent.mouseEnter(indicator);
		expect(screen.queryByRole('dialog', { name: 'Audio plan' })).not.toBeInTheDocument();
	});

	it('says why the Opus compatibility page did not open', async () => {
		const user = userEvent.setup();
		const file = audioFile('/books/part1.mp3', { status: 'ready', codecLabel: 'MP3' });
		const { runtime } = await start({ files: [file] });
		await runtime.input.importIntent({ type: 'importPaths', paths: [file.path] });
		await waitFor(() => expect(runtime.input.view().files).toHaveLength(1));
		engine.seedTitleAudio(file.path, { ...titleAudioRequest(), format: 'm4aOpus' });
		engine.change((state) => {
			state.audio.titles[file.path].facts.codecFamily = 'opus';
		});
		const openUrl = vi.spyOn(tauriClient, 'openUrl').mockRejectedValueOnce({
			code: 'opener_failed',
			category: 'io',
			message: 'No browser could be started.',
			detail: null,
		});
		const indicator = screen.getByRole('button', { name: 'Audio plan for part1.mp3' });
		await user.click(indicator);
		const popup = await screen.findByRole('dialog', { name: 'Audio plan' });

		await user.click(within(popup).getByRole('button', { name: 'Player compatibility' }));

		expect(openUrl).toHaveBeenCalledTimes(1);
		expect(await within(popup).findByText('No browser could be started.')).toBeInTheDocument();
	});

	it('handles keyboard actions only from the focused listbox', async () => {
		const { runtime } = await start({
			files: [
				audioFile('/books/alpha.m4b', { status: 'ready' }),
				audioFile('/books/bravo.m4b', { status: 'ready' }),
			],
		});
		engine.respond = (intent) => {
			if (intent.kind === 'selectFile') return selects([0], 0);
			if (intent.kind === 'selectAll') return selects([0, 1], 0);
			return undefined;
		};
		void runtime.input.importIntent({
			type: 'importPaths',
			paths: ['/books/alpha.m4b', '/books/bravo.m4b'],
		});
		const listbox = await screen.findByRole('listbox', { name: 'Audio files' });
		await waitFor(() => {
			expect(within(listbox).getAllByRole('option')).toHaveLength(2);
		});
		fireEvent.keyDown(listbox, { key: 'ArrowDown' });
		await waitFor(() => {
			expect(runtime.input.view().selectedIndices).toEqual([0]);
		});

		fireEvent.keyDown(document.body, { key: 'ArrowDown' });
		expect(runtime.input.view().selectedIndices).toEqual([0]);

		fireEvent.keyDown(listbox, { key: 'a', ctrlKey: true });
		await waitFor(() => {
			expect(runtime.input.view().selectedIndices).toEqual([0, 1]);
		});
	});

	it('routes cover-art drops away from import and imports file-area drops', async () => {
		const cover = document.createElement('div');
		cover.id = 'cover-art-area';
		document.body.appendChild(cover);
		onTestFinished(() => cover.remove());
		cover.getBoundingClientRect = () => ({
			left: 0,
			right: 100,
			top: 0,
			bottom: 100,
			width: 100,
			height: 100,
			x: 0,
			y: 0,
			toJSON: () => ({}),
		});

		const { input } = await start();
		await waitFor(() => {
			expect(input.dropListening()).toBe(true);
		});

		const container = document.querySelector('.file-management-container') as HTMLElement;
		container.getBoundingClientRect = () => ({
			left: 150,
			right: 400,
			top: 150,
			bottom: 350,
			width: 250,
			height: 200,
			x: 150,
			y: 150,
			toJSON: () => ({}),
		});

		input.drop(['/tmp/image.png'], { x: 50, y: 50 });
		await waitFor(() => {
			expect(engine.sessionIntents).toContainEqual({
				kind: 'loadCoverFromDrop',
				paths: ['/tmp/image.png'],
			});
		});
		expect(engine.analyze).not.toHaveBeenCalled();

		input.drop(['/tmp/file1.wav'], { x: 200, y: 200 });
		await waitFor(() => {
			expect(engine.analyze).toHaveBeenCalledWith(['/tmp/file1.wav']);
		});
	});

	it('shows the engine order lock and its refused-import notice', async () => {
		await start();
		engine.change((state) => {
			state.titles.orderLocked = true;
			state.titles.notice = { kind: 'orderLocked' };
		});
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
			expect(screen.getByText(/Wait for completion to add files/)).toBeInTheDocument();
		});
	});
});
