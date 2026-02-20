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
		viewStateModule.resetStatusPanelViewState();
	});

	it('syncs cover art changes into reactive view state', () => {
		const dataUrl = 'data:image/png;base64,Zm9v';

		domModule.displayCoverArt(dataUrl);
		expect(viewStateModule.statusPanelViewState.coverArtDataUrl).toBe(dataUrl);

		domModule.resetArtThumbnail();
		expect(viewStateModule.statusPanelViewState.coverArtDataUrl).toBeNull();
	});

	it('does not overwrite transient user status text until lock expires', () => {
		const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(1_000);
		domModule.pushTransientStatusMessage('Metadata saved!', 10_000);
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Metadata saved!');

		domModule.updateStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Metadata saved!');

		nowSpy.mockReturnValue(11_001);
		domModule.updateStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Idle');
		nowSpy.mockRestore();
	});

	it('clears transient lock explicitly when requested', () => {
		domModule.pushTransientStatusMessage('Saving…', 10_000);
		domModule.clearTransientStatusMessageLock();
		domModule.updateStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Idle');
	});
});
