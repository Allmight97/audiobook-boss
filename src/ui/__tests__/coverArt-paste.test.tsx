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
		engine.loadTitles([audioFile('/books/a.m4b')], [0]);
		runtime = await createAppRuntime({ engine });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<CoverArtView />
			</AppRuntimeProvider>
		));
		screen.getByTestId('cover-art-area').dispatchEvent(new MouseEvent('mouseover'));
		runtime.metadata.setCoverHovered(true);
	}

	function paste(text: string): void {
		const event = new Event('paste', { bubbles: true, cancelable: true });
		Object.defineProperty(event, 'clipboardData', { value: { getData: () => text } });
		document.body.dispatchEvent(event);
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

	it('says pasted text that is not a URL cannot be a cover', async () => {
		await renderCover();
		paste('just some words');
		await vi.waitFor(() =>
			expect(runtime?.metadata.view().cover.message).toEqual({
				kind: 'error',
				text: 'The pasted text is not an image URL.',
			}),
		);
		expect(engine.sessionIntents.some((intent) => intent.kind === 'loadCoverFromUrl')).toBe(false);
	});
});
