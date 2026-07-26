import { beforeEach, describe, expect, it, vi } from 'vitest';

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
	let viewStateModule: ViewStateModule;

	beforeEach(async () => {
		setupDom();
		vi.resetModules();
		viewStateModule = await import('../viewState.svelte');
		viewStateModule.resetStatusPanelViewState();
	});

	it('does not overwrite transient user status text until lock expires', () => {
		const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(1_000);
		viewStateModule.pushTransientStatusMessage('Metadata saved!', 10_000);
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Metadata saved!');

		viewStateModule.setStatusPanelStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Metadata saved!');

		nowSpy.mockReturnValue(11_001);
		viewStateModule.setStatusPanelStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Idle');
		nowSpy.mockRestore();
	});

	it('clears transient lock explicitly when requested', () => {
		viewStateModule.pushTransientStatusMessage('Saving…', 10_000);
		viewStateModule.clearTransientStatusMessageLock();
		viewStateModule.setStatusPanelStatusText('Idle');
		expect(viewStateModule.statusPanelViewState.statusText).toBe('Idle');
	});
});
