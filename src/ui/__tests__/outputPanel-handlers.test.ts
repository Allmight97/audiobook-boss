import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { handleNamingTemplateInput, resetOutputPanelHandlers } from '../outputPanel/handlers';
import { updateOutputPath } from '../outputPanel/dom';
import { updateNamingTemplate } from '../outputPanel/state.svelte';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		open: vi.fn(),
	},
}));

vi.mock('../outputPanel/dom', () => ({
	updateOutputPath: vi.fn(),
	updateNamingOptionState: vi.fn(),
	showOutputError: vi.fn(),
}));

vi.mock('../outputPanel/state.svelte', () => ({
	updateOutputDirectory: vi.fn(),
	updateNamingPreset: vi.fn(),
	updateNamingTemplate: vi.fn(),
	updateAbsIncludeYear: vi.fn(),
}));

describe('output panel handlers', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.runOnlyPendingTimers();
		vi.useRealTimers();
	});

	it('debounces output preview updates while typing template input', () => {
		const asInputEvent = (target: HTMLInputElement): Event => ({ target }) as unknown as Event;
		const input = document.createElement('input');
		input.value = '{author}';
		handleNamingTemplateInput(asInputEvent(input));

		input.value = '{author}/{series}';
		handleNamingTemplateInput(asInputEvent(input));

		input.value = '{author}/{series}/{title}';
		handleNamingTemplateInput(asInputEvent(input));

		expect(vi.mocked(updateNamingTemplate)).toHaveBeenCalledTimes(3);
		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();

		vi.advanceTimersByTime(149);
		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();

		vi.advanceTimersByTime(1);
		expect(vi.mocked(updateOutputPath)).toHaveBeenCalledTimes(1);
	});

	it('clears pending debounced preview when handlers are re-initialized', () => {
		const asInputEvent = (target: HTMLInputElement): Event => ({ target }) as unknown as Event;
		const input = document.createElement('input');
		input.value = '{author}/{title}';
		handleNamingTemplateInput(asInputEvent(input));

		resetOutputPanelHandlers();
		vi.advanceTimersByTime(150);

		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();
	});
});
