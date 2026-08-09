import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import LeftColumnIsland from '../leftColumn/LeftColumnIsland.svelte';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		analyzeAudioFiles: vi.fn(),
		getRuntimeSettingsCapabilities: vi.fn(async () => ({
			maxConcurrentJobs: { selection: 'auto', effective: 2, allowedValues: [1, 2] },
			encoderSettings: {
				encoders: [],
				bitrateKbpsOptions: [],
				vbrQualityRange: { min: 1, max: 5 },
			},
		})),
		listen: vi.fn(async () => () => undefined),
		openDirectory: vi.fn(),
		openFiles: vi.fn(),
	},
}));

vi.mock('../statusPanel', () => ({
	initStatusPanel: vi.fn(),
	isStatusPanelProcessing: vi.fn(() => false),
	pushStatusPanelTransientStatus: vi.fn(),
}));

describe('left column composition', () => {
	it('renders input workflow before the selected-file inspector as sibling zones', () => {
		render(LeftColumnIsland);

		const shell = screen.getByTestId('left-column');
		const workflow = screen.getByTestId('input-workflow-panel');
		const inspector = screen.getByTestId('file-inspector-panel');

		expect(workflow.parentElement).toBe(shell);
		expect(inspector.parentElement).toBe(shell);
		expect(
			workflow.compareDocumentPosition(inspector) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();
		expect(screen.getByRole('region', { name: 'Input and File Order' })).toBe(workflow);
		expect(screen.getByRole('region', { name: 'Selected File Properties' })).toBe(inspector);
	});
});
