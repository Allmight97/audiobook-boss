import { cleanup, render, screen, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SupportedAudioImportMetadata } from '../../types/audio';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { App } from '../App';

const support: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['m4b'],
	formatsText: 'M4B',
	supportText: 'Supports M4B audio files',
};

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/alpha.m4b']),
		openDirectory: vi.fn(async () => null),
		getSupportedAudioImportMetadata: vi.fn(async () => support),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		...overrides,
	};
}

function engineWithTaggedBooks({ cover = true } = {}): FakeEngine {
	const engine = createFakeEngine();
	engine.analyze = (paths) =>
		paths.map((path) => {
			const title = path.includes('beta') ? 'Beta' : 'Alpha';
			engine.tags.set(path, cover ? { title, cover_art: [0x89, 0x50, 0x4e, 0x47] } : { title });
			return audioFile(path, { duration: 1, size: 1000, tagTitle: title });
		});
	return engine;
}

async function renderApp(runtime: AppRuntime) {
	const rendered = render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	));
	await screen.findByTestId('left-column');
	return rendered;
}

function lastIntents(engine: FakeEngine, count: number) {
	return engine.sessionIntents.slice(-count);
}

describe('metadata workbench shell', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it.each([
		{ name: 'while a cover shows', cover: true, tabIndex: 0 },
		{ name: 'not when no cover shows', cover: false, tabIndex: -1 },
	])('lets Tab reach Clear cover $name', async ({ cover, tabIndex }) => {
		const engine = engineWithTaggedBooks({ cover });
		runtime = createAppRuntime({ input: fakeInput(), engine });
		await renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement | null)?.value).toBe(
				'Alpha',
			);
		});
		expect((document.getElementById('cover-art-clear-btn') as HTMLButtonElement).tabIndex).toBe(
			tabIndex,
		);
	});

	it('sends a typed title and the save to the engine in that order', async () => {
		const engine = engineWithTaggedBooks();
		runtime = createAppRuntime({ input: fakeInput(), engine });
		await renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		title.focus();
		await userEvent.clear(title);
		await userEvent.type(title, 'Edited');
		engine.respond = (intent) => {
			if (intent.kind !== 'save') return undefined;
			engine.status({
				kind: 'saveComplete',
				succeeded: 1,
				failed: 0,
				cancelled: 0,
				waiting: 0,
				held: 0,
				outputs: { updated: 0, elsewhere: 0, restartOffered: 0, failed: 0 },
			});
			return { kind: 'applied' };
		};
		await userEvent.click(screen.getByTestId('metadata-save-btn'));
		await waitFor(() => expect(lastIntents(engine, 2)[1]).toEqual({ kind: 'save' }));
		expect(lastIntents(engine, 2)[0]).toEqual({
			kind: 'setField',
			field: 'title',
			value: 'Edited',
		});
		expect(title.value).toBe('Edited');
		expect(screen.getByTestId('metadata-status-message')).toHaveTextContent(
			'Metadata save complete: success=1, failed=0, cancelled=0',
		);
	});

	it('saves from the global shortcut', async () => {
		const engine = engineWithTaggedBooks();
		runtime = createAppRuntime({ input: fakeInput(), engine });
		await renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		await userEvent.type(title, ' Two');
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 's', metaKey: true, bubbles: true }));
		await waitFor(() =>
			expect(lastIntents(engine, 2)).toEqual([
				{ kind: 'setField', field: 'title', value: 'Alpha Two' },
				{ kind: 'save' },
			]),
		);
	});
});
