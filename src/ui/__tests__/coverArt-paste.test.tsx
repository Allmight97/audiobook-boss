import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { CoverArtView } from '../coverArt';

describe('cover art paste', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function renderCover() {
		engine = createFakeEngine();
		engine.loadTitles([audioFile('/books/a.m4b', { status: 'ready' })], [0]);
		runtime = await createAppRuntime({ engine });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<CoverArtView />
			</AppRuntimeProvider>
		));
		screen.getByTestId('cover-art-area').dispatchEvent(new MouseEvent('mouseover'));
		runtime.metadata.setCoverHovered(true);
	}

	function paste(text: string, target: EventTarget = document.body): void {
		const event = new Event('paste', { bubbles: true, cancelable: true });
		Object.defineProperty(event, 'clipboardData', { value: { getData: () => text } });
		target.dispatchEvent(event);
	}

	it('loads a pasted URL over the cover', async () => {
		await renderCover();
		paste('https://example.com/cover.jpg');
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'loadCoverFromUrl',
				url: 'https://example.com/cover.jpg',
			}),
		);
	});

	it('sends pasted text over the cover to the engine whatever it looks like', async () => {
		await renderCover();
		paste('  just some words ');
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'loadCoverFromUrl',
				url: 'just some words',
			}),
		);
	});

	it('leaves pasted text to the URL field and says nothing when it is not a URL', async () => {
		await renderCover();
		const field = screen.getByTestId('cover-art-url-input');
		field.focus();
		paste('example.com/cover', field);
		await Promise.resolve();
		expect(runtime?.metadata.view().cover.message).toEqual({ kind: 'hidden' });
		expect(engine.sessionIntents.some((intent) => intent.kind === 'loadCoverFromUrl')).toBe(false);
	});

	it('still loads a URL pasted into the URL field', async () => {
		await renderCover();
		const field = screen.getByTestId('cover-art-url-input');
		field.focus();
		paste('https://example.com/cover.jpg', field);
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'loadCoverFromUrl',
				url: 'https://example.com/cover.jpg',
			}),
		);
	});
});
