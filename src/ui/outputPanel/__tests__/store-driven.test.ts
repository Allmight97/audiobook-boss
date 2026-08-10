import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/svelte';
import OutputPanelIsland from '../OutputPanelIsland.svelte';
import { initOutputPanel } from '../index';
import { updateEstimatedSize } from '../preview';
import {
	outputPanelState,
	readOutputRequestConfig,
	updateAbsIncludeYear,
	updateNamingPreset,
	updateNamingTemplate,
	updateOutputDirectory,
} from '../state.svelte';
import { encoderPanelState, resetEncoderPanelState } from '../../encoderPanel/state.svelte';

const context = vi.hoisted(() => ({
	getCurrentFileListMock: vi.fn(),
}));

vi.mock('../../fileList/state.svelte', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	getSelectedFiles: vi.fn(() => []),
	isOrderLocked: vi.fn(() => false),
	onOrderLockChange: vi.fn(() => () => undefined),
}));

describe('output panel state-driven contracts', () => {
	beforeEach(() => {
		context.getCurrentFileListMock.mockReset();
		context.getCurrentFileListMock.mockReturnValue({
			files: [{ path: '/books/a.m4b', isValid: true }],
			totalDuration: 3600,
		});

		document.body.innerHTML = `
			<span id="estimated-size"></span>
			<input id="meta-title" value="" />
			<input id="meta-author" value="" />
			<input id="meta-narrator" value="" />
			<input id="meta-year" value="" />
			<input id="meta-genre" value="" />
			<textarea id="meta-description"></textarea>
			<input id="meta-series" value="" />
			<input id="meta-series-part" value="" />
			<input id="meta-subseries" value="" />
			<input id="meta-subseries-part" value="" />
			<div id="meta-series-part-warning" hidden></div>
			<div id="meta-subseries-part-warning" hidden></div>
		`;
		resetEncoderPanelState();
		updateOutputDirectory('/tmp/out');
		updateNamingPreset('absDefault');
		updateNamingTemplate('');
		updateAbsIncludeYear(false);
	});

	it('renders the initial output choice through the owned island', () => {
		updateOutputDirectory('');
		render(OutputPanelIsland);
		initOutputPanel();

		expect(document.getElementById('output-preview-text')?.textContent).toBe(
			'Select output directory...',
		);
		expect(document.getElementById('output-dir-browse')).toBeTruthy();
	});

	it('renders usable naming and ABS controls through the owned island', async () => {
		// Keep this rendered interaction focused on the control surface rather than
		// the asynchronous preview path request.
		updateOutputDirectory('');
		render(OutputPanelIsland);

		const namingPreset = screen.getByRole('combobox', { name: 'Naming preset' });
		expect(namingPreset).toHaveValue('absDefault');
		expect(
			Array.from((namingPreset as HTMLSelectElement).options).map((option) => option.text),
		).toEqual(['ABS Default', 'Custom Template']);

		const includeYear = screen.getByRole('checkbox', {
			name: 'Include year segment (YYYY)',
		});
		expect(includeYear).toBeEnabled();
		expect(includeYear).not.toBeChecked();
		await fireEvent.click(includeYear);
		expect(includeYear).toBeChecked();
		expect(outputPanelState.absIncludeYear).toBe(true);
		expect(
			screen.getByText('Creates Author / Series / (Sub-series) / Book # - YYYY - Title'),
		).toBeVisible();

		await fireEvent.change(namingPreset, { target: { value: 'customTemplate' } });
		expect(namingPreset).toHaveValue('customTemplate');
		const templateRow = document.getElementById('output-template-row');
		expect(templateRow).not.toHaveAttribute('hidden');

		const templateInput = screen.getByRole('textbox', { name: 'Template' });
		expect(templateInput).toBeEnabled();
		await fireEvent.input(templateInput, { target: { value: '{author}/{title}' } });
		expect(templateInput).toHaveValue('{author}/{title}');
		expect(outputPanelState.namingTemplate).toBe('{author}/{title}');
	});

	it('reads output request config from canonical state selector', () => {
		const config = readOutputRequestConfig();

		expect(config).toMatchObject({
			outputDirectory: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
	});

	it('defaults custom template when custom preset is selected without template text', () => {
		updateNamingPreset('customTemplate');
		updateNamingTemplate('   ');

		const config = readOutputRequestConfig();
		expect(config.outputNaming).toMatchObject({
			preset: 'customTemplate',
			customTemplate: '{author}/{title}',
		});
	});

	it('updates estimated size from encoder panel public state', () => {
		encoderPanelState.bitrateValue = 48;
		encoderPanelState.channelsSelection = 'mono';
		updateEstimatedSize();
		const lowValue = outputPanelState.estimatedSizeText;

		encoderPanelState.bitrateValue = 128;
		encoderPanelState.channelsSelection = 'stereo';
		updateEstimatedSize();
		const highValue = outputPanelState.estimatedSizeText;

		const lowMegabytes = Number.parseFloat(lowValue.replace(/[^\d.]+/g, ''));
		const highMegabytes = Number.parseFloat(highValue.replace(/[^\d.]+/g, ''));

		expect(lowMegabytes).toBeGreaterThan(0);
		expect(highMegabytes).toBeGreaterThan(lowMegabytes);
		expect(highMegabytes).toBeGreaterThan(lowMegabytes * 3);
	});
});
