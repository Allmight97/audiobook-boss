import { beforeEach, describe, expect, it, vi } from 'vitest';

type FeedbackModule = typeof import('../feedback');
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

describe('statusPanel feedback and view-state updates', () => {
	let feedbackModule: FeedbackModule;
	let viewStateModule: ViewStateModule;

	beforeEach(async () => {
		setupDom();
		vi.resetModules();
		feedbackModule = await import('../feedback');
		viewStateModule = await import('../viewState.svelte');
		viewStateModule.resetStatusPanelViewState();
	});

	it('syncs cover art changes into reactive view state', () => {
		const dataUrl = 'data:image/png;base64,Zm9v';

		feedbackModule.displayCoverArt(dataUrl);
		expect(viewStateModule.statusPanelViewState.coverArtDataUrl).toBe(dataUrl);

		feedbackModule.resetArtThumbnail();
		expect(viewStateModule.statusPanelViewState.coverArtDataUrl).toBeNull();
	});

	it('does not overwrite transient user status text until lock expires', () => {
		const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(1_000);
		feedbackModule.pushTransientStatusMessage('Metadata saved!', 10_000);
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Metadata saved!');

		feedbackModule.updateStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Metadata saved!');

		nowSpy.mockReturnValue(11_001);
		feedbackModule.updateStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Idle');
		nowSpy.mockRestore();
	});

	it('clears transient lock explicitly when requested', () => {
		feedbackModule.pushTransientStatusMessage('Saving…', 10_000);
		feedbackModule.clearTransientStatusMessageLock();
		feedbackModule.updateStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Idle');
	});
});
