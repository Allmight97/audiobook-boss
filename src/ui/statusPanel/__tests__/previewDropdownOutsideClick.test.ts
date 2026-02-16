import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as dom from '../dom';
import { bindStatusPanelDomEvents } from '../events';

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

    <button id="preview-button">Preview</button>
    <button id="preview-dropdown-toggle">Toggle Preview Duration</button>
    <div id="preview-dropdown" style="display: none;">
      <button class="split-option" data-duration="45">45s</button>
      <div id="preview-dropdown-inside-target">Inside target</div>
    </div>
    <div id="outside-target">Outside target</div>
  `;
}

function createHandlers() {
	return {
		onProcess: vi.fn(async () => undefined),
		onCancelAll: vi.fn(async () => undefined),
		onPreview: vi.fn(async (_duration: number) => undefined),
		getPreviewDuration: vi.fn(() => 30),
		setPreviewDuration: vi.fn((_duration: number) => undefined),
		onUpdateConcurrencyIndicator: vi.fn(() => undefined),
	};
}

describe('preview dropdown outside-click behavior', () => {
	beforeEach(() => {
		setupDom();
		dom.resetStatusPanelDomCache();
	});

	it('keeps dropdown open when clicking inside non-option content', () => {
		bindStatusPanelDomEvents(createHandlers());

		const dropdown = document.getElementById('preview-dropdown') as HTMLDivElement;
		const insideTarget = document.getElementById(
			'preview-dropdown-inside-target',
		) as HTMLDivElement;
		dropdown.style.display = 'block';

		insideTarget.click();

		expect(dropdown.style.display).toBe('block');
	});

	it('toggles dropdown visibility from the toggle button', () => {
		bindStatusPanelDomEvents(createHandlers());

		const dropdown = document.getElementById('preview-dropdown') as HTMLDivElement;
		const toggle = document.getElementById('preview-dropdown-toggle') as HTMLButtonElement;
		expect(dropdown.style.display).toBe('none');

		toggle.click();
		expect(dropdown.style.display).toBe('block');

		toggle.click();
		expect(dropdown.style.display).toBe('none');
	});

	it('closes dropdown when clicking outside toggle and dropdown', () => {
		bindStatusPanelDomEvents(createHandlers());

		const dropdown = document.getElementById('preview-dropdown') as HTMLDivElement;
		const outsideTarget = document.getElementById('outside-target') as HTMLDivElement;
		dropdown.style.display = 'block';

		outsideTarget.click();

		expect(dropdown.style.display).toBe('none');
	});

	it('closes dropdown and triggers preview when selecting an option', () => {
		const handlers = createHandlers();
		bindStatusPanelDomEvents(handlers);

		const dropdown = document.getElementById('preview-dropdown') as HTMLDivElement;
		const option = document.querySelector('.split-option') as HTMLButtonElement;
		dropdown.style.display = 'block';

		option.click();

		expect(handlers.setPreviewDuration).toHaveBeenCalledOnce();
		expect(handlers.setPreviewDuration).toHaveBeenCalledWith(45);
		expect(handlers.onPreview).toHaveBeenCalledOnce();
		expect(handlers.onPreview).toHaveBeenCalledWith(45);
		expect(dropdown.style.display).toBe('none');
	});
});
