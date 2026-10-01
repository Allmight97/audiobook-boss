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
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
		...overrides,
	};
}

/** An engine whose files carry a title and a PNG cover. */
function engineWithTaggedBooks(): FakeEngine {
	const engine = createFakeEngine();
	engine.analyze = (paths) =>
		paths.map((path) => {
			const title = path.includes('beta') ? 'Beta' : 'Alpha';
			engine.tags.set(path, { title, cover_art: [0x89, 0x50, 0x4e, 0x47] });
			return audioFile(path, { duration: 1, size: 1000, tagTitle: title });
		});
	return engine;
}

function renderApp(runtime: AppRuntime) {
	return render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	));
}

describe('metadata workbench shell', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it('composes cover and form zones and keeps cover clear keyboard-reachable', async () => {
		const engine = engineWithTaggedBooks();
		runtime = createAppRuntime({ input: fakeInput(), engine });
		renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect(screen.getByTestId('metadata-manager')).toBeTruthy();
		});
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement | null)?.value).toBe(
				'Alpha',
			);
		});
		expect(screen.getByTestId('cover-art-area')).toBeTruthy();
		const loadButton = screen.getByTestId('cover-art-url-load-btn');
		const findMetadata = screen.getByTestId('metadata-lookup-btn');
		expect(loadButton.className.split(/\s+/)).toEqual(
			expect.arrayContaining(['abb-button', 'abb-button-secondary', 'cover-art-url-load-btn']),
		);
		expect(findMetadata.className.split(/\s+/)).toEqual(
			expect.arrayContaining(['abb-button', 'abb-button-secondary']),
		);
		expect(screen.queryByTestId('metadata-artifacts')).toBeNull();
		const clearButton = document.getElementById('cover-art-clear-btn') as HTMLButtonElement;
		expect(clearButton.tabIndex).toBe(0);
		expect(getComputedStyle(clearButton).display).not.toBe('none');
		clearButton.focus();
		expect(document.activeElement).toBe(clearButton);
	});

	it('sends a typed title and the save to the engine in that order', async () => {
		const engine = engineWithTaggedBooks();
		runtime = createAppRuntime({ input: fakeInput(), engine });
		renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		title.focus();
		await userEvent.clear(title);
		await userEvent.type(title, 'Edited');
		await userEvent.click(screen.getByTestId('metadata-save-btn'));
		await waitFor(() => {
			expect(engine.saves).toEqual([
				{
					'/books/alpha.m4b': {
						title: { op: 'set', value: 'Edited' },
						album: { op: 'set', value: 'Edited' },
					},
				},
			]);
		});
		expect(title.value).toBe('Edited');
		expect(screen.getByTestId('metadata-status-message')).toHaveTextContent(
			'Metadata save complete: success=1, failed=0, cancelled=0',
		);
	});

	it('saves from the global shortcut', async () => {
		const engine = engineWithTaggedBooks();
		runtime = createAppRuntime({ input: fakeInput(), engine });
		renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		await userEvent.type(title, ' Two');
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 's', metaKey: true, bubbles: true }));
		await waitFor(() => {
			expect(engine.saves[0]?.['/books/alpha.m4b']?.title).toEqual({
				op: 'set',
				value: 'Alpha Two',
			});
		});
	});
});
