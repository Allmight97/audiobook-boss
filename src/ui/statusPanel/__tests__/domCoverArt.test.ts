import { beforeEach, describe, expect, it, vi } from 'vitest';

type DomModule = typeof import('../dom');

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

	beforeEach(async () => {
		setupDom();
		vi.resetModules();
		domModule = await import('../dom');
	});

	it('renders one img for cover art and resets to the placeholder span', () => {
		const dataUrl = 'data:image/png;base64,Zm9v';

		domModule.displayCoverArt(dataUrl);

		const artThumbnail = document.querySelector('.art-thumbnail') as HTMLElement;
		const images = artThumbnail.querySelectorAll('img');
		expect(images).toHaveLength(1);
		expect(images[0].getAttribute('src')).toBe(dataUrl);
		expect(images[0].getAttribute('alt')).toBe('Cover Art');
		expect(artThumbnail.children).toHaveLength(1);

		domModule.resetArtThumbnail();

		const placeholderSpans = artThumbnail.querySelectorAll('span');
		expect(artThumbnail.querySelectorAll('img')).toHaveLength(0);
		expect(placeholderSpans).toHaveLength(1);
		expect(placeholderSpans[0].textContent).toBe('Art');
		expect(artThumbnail.children).toHaveLength(1);
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
