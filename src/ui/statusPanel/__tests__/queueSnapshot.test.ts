import { beforeEach, describe, expect, it } from 'vitest';
import * as dom from '../dom';
import { StatusPanel } from '../logic';

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
    <select id="max-concurrent-select"></select>
  `;
}

function getJobRows(): string[] {
	return Array.from(document.querySelectorAll<HTMLElement>('#job-list span')).map(
		(node) => node.textContent ?? '',
	);
}

async function flushRenderFrame(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => resolve());
	});
}

describe('StatusPanel queue snapshot', () => {
	beforeEach(() => {
		setupDom();
		dom.resetStatusPanelDomCache();
	});

	it('initializes queued items in order', () => {
		const panel = new StatusPanel();
		(panel as any).handleQueueSnapshot({
			items: [
				{ input_index: 0, file_path: '/books/alpha.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
			],
			max_concurrent: 2,
		});

		expect(getJobRows()).toEqual(['alpha.m4b • Queued • #1 of 2', 'beta.m4b • Queued • #2 of 2']);
		expect((document.getElementById('status-text') as HTMLElement).textContent).toBe('Analyzing');
		expect((document.getElementById('step-text') as HTMLElement).textContent).toBe(
			'Current Step: Queued 2 files',
		);
		expect(panel.getCurrentStatus()).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 2 files',
		});

		// Secondary internal check: queue order contract remains stable.
		expect((panel as any).queueOrder).toEqual(['idx:0', 'idx:1']);
	});

	it('applies queue snapshot order/labels after early progress arrives', async () => {
		const panel = new StatusPanel();

		panel.updateProgress({
			input_index: 1,
			stage: 'converting',
			percentage: 45,
			message: 'working',
		} as any);
		await flushRenderFrame();

		expect(getJobRows()).toHaveLength(1);
		expect(getJobRows()[0]).toContain('Converting (45.0%)');
		expect((document.getElementById('step-text') as HTMLElement).textContent).toBe(
			'Current Step: working',
		);

		(panel as any).handleQueueSnapshot({
			items: [
				{ input_index: 2, file_path: '/books/gamma.m4b' },
				{ input_index: 1, file_path: '/books/beta.m4b' },
				{ input_index: 0, file_path: '/books/alpha.m4b' },
			],
			max_concurrent: 2,
		});

		expect(getJobRows()).toEqual([
			'gamma.m4b • Queued • #1 of 3',
			'beta.m4b • Queued • #2 of 3',
			'alpha.m4b • Queued • #3 of 3',
		]);
		expect((document.getElementById('step-text') as HTMLElement).textContent).toBe(
			'Current Step: Queued 3 files',
		);
		expect(panel.getCurrentStatus()).toEqual({
			stage: 'analyzing',
			percentage: 0,
			message: 'Queued 3 files',
		});

		// Secondary internal check: snapshot order tracking still follows backend order.
		expect((panel as any).queueOrder).toEqual(['idx:2', 'idx:1', 'idx:0']);
	});
});
