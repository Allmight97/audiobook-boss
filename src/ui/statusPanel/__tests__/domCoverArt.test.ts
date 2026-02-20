import { beforeEach, describe, expect, it, vi } from 'vitest';

type DomModule = typeof import('../dom');
type ViewStateModule = typeof import('../viewState.svelte');

function setupDom() {
	document.body.innerHTML = `
    <div id="progress-bar"></div>
    <div id="percentage-processed"></div>
    <div id="status-text"></div>
    <div id="step-text"></div>
    <div id="concurrency-status"></div>
    <button id="process-button"></button>
    <button id="cancel-all-button"></button>
    <div class="art-thumbnail"></div>
    <div id="job-list"></div>
  `;
}

describe('statusPanel cover art DOM rendering', () => {
	let domModule: DomModule;
	let viewStateModule: ViewStateModule;

	beforeEach(async () => {
		setupDom();
		vi.resetModules();
		domModule = await import('../dom');
		viewStateModule = await import('../viewState.svelte');
	});

	it('syncs cover art changes into reactive view state', () => {
		const dataUrl = 'data:image/png;base64,Zm9v';

		domModule.displayCoverArt(dataUrl);
		expect(viewStateModule.statusPanelViewState.coverArtDataUrl).toBe(dataUrl);

		domModule.resetArtThumbnail();
		expect(viewStateModule.statusPanelViewState.coverArtDataUrl).toBeNull();
	});

	it('does not overwrite user-locked status text until lock expires', () => {
		const statusText = document.getElementById('status-text') as HTMLElement;
		statusText.dataset.userStatusLockUntil = String(Date.now() + 10_000);
		statusText.textContent = 'Metadata saved!';

		domModule.updateStatusText('Idle');
		expect(statusText.textContent).toBe('Metadata saved!');

		statusText.dataset.userStatusLockUntil = String(Date.now() - 1);
		domModule.updateStatusText('Idle');
		expect(statusText.textContent).toBe('Idle');
	});
});
