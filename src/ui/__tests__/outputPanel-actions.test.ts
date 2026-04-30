import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	browseOutputDirectory,
	editNamingTemplate,
	resetOutputPanelActions,
	selectNamingPreset,
	setAbsIncludeYear,
} from '../outputPanel/actions';
import { updateNamingOptionState, updateOutputPath } from '../outputPanel/preview';
import {
	updateAbsIncludeYear,
	updateNamingPreset,
	updateNamingTemplate,
	updateOutputDirectory,
} from '../outputPanel/state.svelte';
import { tauriClient } from '../../lib/tauri/client';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		open: vi.fn(),
	},
}));

vi.mock('../outputPanel/preview', () => ({
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

describe('output panel actions', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.clearAllMocks();
	});

	afterEach(() => {
		vi.runOnlyPendingTimers();
		vi.useRealTimers();
	});

	it('debounces output preview updates while typing template input', () => {
		editNamingTemplate('{author}');
		editNamingTemplate('{author}/{series}');
		editNamingTemplate('{author}/{series}/{title}');

		expect(vi.mocked(updateNamingTemplate)).toHaveBeenCalledTimes(3);
		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();

		vi.advanceTimersByTime(149);
		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();

		vi.advanceTimersByTime(1);
		expect(vi.mocked(updateOutputPath)).toHaveBeenCalledTimes(1);
	});

	it('clears pending debounced preview when handlers are re-initialized', () => {
		editNamingTemplate('{author}/{title}');

		resetOutputPanelActions();
		vi.advanceTimersByTime(150);

		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();
	});

	it('normalizes a selected output directory and refreshes the preview', async () => {
		vi.mocked(tauriClient.open).mockResolvedValueOnce(['/books/out']);

		await browseOutputDirectory();

		expect(vi.mocked(updateOutputDirectory)).toHaveBeenCalledWith('/books/out');
		expect(vi.mocked(updateOutputPath)).toHaveBeenCalledTimes(1);
	});

	it('ignores cancelled output directory selection', async () => {
		vi.mocked(tauriClient.open).mockResolvedValueOnce(null);

		await browseOutputDirectory();

		expect(vi.mocked(updateOutputDirectory)).not.toHaveBeenCalled();
		expect(vi.mocked(updateOutputPath)).not.toHaveBeenCalled();
	});

	it('maps naming controls from values before refreshing preview state', () => {
		selectNamingPreset('customTemplate');
		setAbsIncludeYear(true);

		expect(vi.mocked(updateNamingPreset)).toHaveBeenCalledWith('customTemplate');
		expect(vi.mocked(updateAbsIncludeYear)).toHaveBeenCalledWith(true);
		expect(vi.mocked(updateNamingOptionState)).toHaveBeenCalledTimes(2);
		expect(vi.mocked(updateOutputPath)).toHaveBeenCalledTimes(2);
	});
});
