import { screen, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine } from '../../test/fixtures/fakeEngine';
import { renderApp } from '../../test/fixtures/renderApp';

describe('Solid import tracer shell', () => {
	it('renders an analyzed local import row from picker intent', async () => {
		const user = userEvent.setup();
		const engine = createFakeEngine();
		engine.analyze = (paths) =>
			paths.map((path) =>
				audioFile(path, {
					status: 'ready',
					inputId: 'input-1',
					duration: 300,
					size: 15 * 1024 * 1024,
					label: 'Chapter One',
					artist: 'Narrator',
				}),
			);
		await renderApp({
			engine,
			input: { openFiles: vi.fn(async () => ['/books/chapter.m4b']) },
		});

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		const row = await screen.findByRole('option', { name: 'Chapter One' });
		expect(within(row).getByText('Chapter One')).toBeInTheDocument();
		expect(within(row).getByText(/Narrator/)).toBeInTheDocument();
		expect(screen.getByRole('region', { name: 'Input and File Order' })).toBeInTheDocument();
		expect(screen.getByRole('region', { name: 'Selected File Properties' })).toBeInTheDocument();
		expect(screen.getByRole('region', { name: 'Metadata Manager' })).toBeInTheDocument();
	});

	it("counts a CUE-backed file's chapters from its CUE plan", async () => {
		const user = userEvent.setup();
		const engine = createFakeEngine();
		const one = { title: 'One', startMs: 0, endMs: 400 };
		const two = { title: 'Two', startMs: 400, endMs: 60_000 };
		engine.analyze = (paths) =>
			paths.map((path) =>
				audioFile(path.replace(/\.m4b$/, '.mp3'), {
					status: 'ready',
					duration: 60,
					chapters: [],
					chapterPlan: { chapters: [one, two], fromCue: true, sourceFingerprint: '1:1' },
					cueSource: {
						fileName: 'chapter.cue',
						status: 'ready',
						message: '',
						actions: ['ignore'],
					},
				}),
			);
		await renderApp({
			engine,
			input: { openFiles: vi.fn(async () => ['/books/chapter.m4b']) },
		});

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		const row = await screen.findByRole('option', { name: 'chapter.mp3' });
		expect(within(row).getByText(/• 2 chapters$/)).toBeInTheDocument();
		expect(within(row).getByText('2 chapters from chapter.cue')).toBeInTheDocument();
	});

	it.each(['ignored', 'invalid'] as const)(
		'names no chapter source for a CUE with status %s',
		async (status) => {
			const user = userEvent.setup();
			const engine = createFakeEngine();
			engine.analyze = (paths) =>
				paths.map((path) =>
					audioFile(path.replace(/\.m4b$/, '.mp3'), {
						status: 'ready',
						chapterPlan: null,
						cueSource: { fileName: 'chapter.cue', status, message: 'About the CUE.', actions: [] },
					}),
				);
			await renderApp({
				engine,
				input: { openFiles: vi.fn(async () => ['/books/chapter.m4b']) },
			});

			await user.click(screen.getByRole('button', { name: 'Add audio files' }));
			const row = await screen.findByRole('option', { name: 'chapter.mp3' });
			expect(within(row).getByText('About the CUE.')).toBeInTheDocument();
			expect(within(row).queryByText(/chapters? from/)).toBeNull();
		},
	);

	it('names a row and its reorder grip by the title label, not the file name', async () => {
		const user = userEvent.setup();
		const engine = createFakeEngine();
		engine.analyze = (paths) =>
			paths.map((path) =>
				audioFile(path, { status: 'ready', label: 'Chapter One', artist: 'Narrator' }),
			);
		await renderApp({
			engine,
			input: { openFiles: vi.fn(async () => ['/books/chapter.m4b']) },
		});

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		const row = await screen.findByRole('option', { name: 'Chapter One' });
		expect(within(row).getByRole('button', { name: 'Reorder Chapter One' })).toBeInTheDocument();
	});
});
