import { tauriClient } from '../../lib/tauri/client';
import { EVENTS } from '../../types/events';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import * as dom from './dom';

interface StatusPanelEventHandlers {
	onProcess: () => Promise<void>;
	onCancelAll: () => Promise<void>;
	onPreview: (duration: number) => Promise<void>;
	getPreviewDuration: () => number;
	setPreviewDuration: (duration: number) => void;
	onUpdateConcurrencyIndicator: () => void;
}

export function bindStatusPanelDomEvents(handlers: StatusPanelEventHandlers): void {
	const processButton = dom.getProcessButton();
	const previewButton = document.getElementById('preview-button') as HTMLButtonElement | null;
	const previewDropdownToggle = document.getElementById(
		'preview-dropdown-toggle',
	) as HTMLButtonElement | null;
	const previewDropdown = document.getElementById('preview-dropdown') as HTMLDivElement | null;
	const advancedToggle = document.getElementById(
		'advanced-settings-toggle',
	) as HTMLButtonElement | null;

	if (processButton) {
		processButton.addEventListener('click', () => {
			void handlers.onProcess();
		});
	}

	const cancelAllButton = dom.getCancelAllButton();
	if (cancelAllButton) {
		cancelAllButton.addEventListener('click', () => {
			void handlers.onCancelAll();
		});
	}

	if (previewButton) {
		previewButton.addEventListener('click', () => {
			void handlers.onPreview(handlers.getPreviewDuration());
		});
	}

	if (previewDropdownToggle && previewDropdown) {
		previewDropdownToggle.addEventListener('click', (event) => {
			event.stopPropagation();
			previewDropdown.style.display = previewDropdown.style.display === 'none' ? 'block' : 'none';
		});

		previewDropdown.querySelectorAll('.split-option').forEach((opt) => {
			opt.addEventListener('click', () => {
				const duration = parseInt((opt as HTMLElement).dataset.duration || '30', 10);
				handlers.setPreviewDuration(duration);
				previewDropdown.style.display = 'none';
				void handlers.onPreview(duration);
			});
		});

		document.addEventListener('click', (event) => {
			const target = event.target;
			if (!(target instanceof Node)) return;
			const clickedOutsideDropdown =
				!previewDropdown.contains(target) && !previewDropdownToggle.contains(target);
			if (clickedOutsideDropdown) {
				previewDropdown.style.display = 'none';
			}
		});
	}

	if (advancedToggle) {
		advancedToggle.addEventListener('click', () => {
			const panel = document.getElementById('advanced-settings-panel');
			const icon = document.getElementById('advanced-toggle-icon');
			if (panel) {
				panel.classList.toggle('open');
				if (icon) icon.textContent = panel.classList.contains('open') ? '▼' : '▶';
			}
		});
	}

	document.addEventListener('abb:max-concurrent-updated', () => {
		handlers.onUpdateConcurrencyIndicator();
	});
}

export async function listenForProgressEvents(
	onProgress: (event: ProcessingProgressEvent) => void,
): Promise<() => void> {
	return tauriClient.listen(EVENTS.PROGRESS, (event) => {
		onProgress(event.payload);
	});
}

export async function listenForQueueEvents(
	onQueue: (event: ProcessingQueueEvent) => void,
): Promise<() => void> {
	return tauriClient.listen(EVENTS.QUEUE, (event) => {
		onQueue(event.payload);
	});
}
