import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import EncodingWorkbenchIsland from '../encodingWorkbench/EncodingWorkbenchIsland.svelte';
import { applyOutputDefaultsFromSettings, updateOutputPath } from '../outputPanel';

const { encoderLogicMocks } = vi.hoisted(() => ({
	encoderLogicMocks: {
		handleBitrateModeChange: vi.fn(),
		handleBitrateValueChange: vi.fn(),
		handleChannelsSelectionChange: vi.fn(),
		handleFlavorChange: vi.fn(),
		handleFdkAfterburnerChange: vi.fn(),
		handleQualityValueChange: vi.fn(),
		handleSampleRateSelectionChange: vi.fn(),
		initializeEncoderPanelLogic: vi.fn(),
	},
}));

vi.mock('../encoderPanel/logic', () => encoderLogicMocks);
vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		previewOutputPath: vi
			.fn()
			.mockResolvedValue(
				'/Users/jstar/Projects/ABB Tests/output/Brandon Sanderson/The Stormlight Archive/1 - The Way of Kings.m4b',
			),
		validateMetadataIntentPatch: vi.fn(async (metadataPatch: unknown) => ({
			isValid: true,
			metadataPatch,
			fieldErrors: [],
		})),
	},
}));

function setupMetadataInputs(): void {
	document.body.innerHTML = `
		<input id="meta-title" value="The Way of Kings" />
		<input id="meta-author" value="Brandon Sanderson" />
		<input id="meta-narrator" value="Michael Kramer, Kate Reading" />
		<input id="meta-series" value="The Stormlight Archive" />
		<input id="meta-series-part" value="1" />
		<input id="meta-subseries" value="" />
		<input id="meta-subseries-part" value="" />
		<input id="meta-year" value="2010" />
		<input id="meta-genre" value="Fantasy" />
	`;
}

describe('EncodingWorkbenchIsland', () => {
	beforeEach(() => {
		setupMetadataInputs();
		for (const mock of Object.values(encoderLogicMocks)) {
			mock.mockReset();
		}
	});

	it('renders encoder, output, and tags as one scoped workbench row', async () => {
		render(EncodingWorkbenchIsland);
		applyOutputDefaultsFromSettings({
			outputDirectory: '/Users/jstar/Projects/ABB Tests/output',
			outputNaming: { preset: 'absDefault', includeYear: true, customTemplate: '' },
		});
		updateOutputPath('final');

		expect(screen.getByTestId('encoding-workbench')).toBeTruthy();
		const encoderBlock = screen.getByTestId('encoding-workbench-encoder');
		const outputBlock = screen.getByTestId('encoding-workbench-output');
		const tagsBlock = screen.getByTestId('encoding-workbench-tags');
		expect(encoderBlock.querySelector('h3')?.textContent).toBe('Encoder');
		expect(outputBlock.querySelector('h3')?.textContent).toBe('Output');
		expect(tagsBlock.querySelector('h3')?.textContent).toBe('Tags Preview');
		await vi.waitFor(() => {
			expect(screen.getByTestId('output-directory-value').textContent).toContain(
				'ABB Tests/output',
			);
			expect(screen.getByTestId('output-example').textContent).toContain('The Way of Kings.m4b');
		});
		expect(encoderLogicMocks.initializeEncoderPanelLogic).toHaveBeenCalledTimes(1);
	});

	it('does not retain Preview Audio in the tags block after its transport relocation', async () => {
		render(EncodingWorkbenchIsland);
		expect(screen.queryByRole('button', { name: 'Preview Audio' })).toBeNull();
	});
});
