import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings } from '../../types/audio';
import { appStore } from '../core/appStore.svelte';
import { updateEstimatedSize } from '../outputPanel/dom';
import {
	readOutputConfigForProcessing,
	updateAbsIncludeYear,
	updateEncoderSettings,
	updateNamingPreset,
	updateNamingTemplate,
	updateOutputDirectory,
	updateSampleRate,
} from '../outputPanel/state';

const context = vi.hoisted(() => ({
	getCurrentFileListMock: vi.fn(),
}));

vi.mock('../fileList', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
}));

describe('output panel store-driven contracts', () => {
	beforeEach(() => {
		context.getCurrentFileListMock.mockReset();
		context.getCurrentFileListMock.mockReturnValue({
			files: [{ path: '/books/a.m4b', isValid: true }],
			totalDuration: 3600,
		});

		document.body.innerHTML = '<span id="estimated-size"></span>';
		updateOutputDirectory('/tmp/out');
		updateNamingPreset('absDefault');
		updateNamingTemplate('');
		updateAbsIncludeYear(false);
		updateSampleRate('auto');
		updateEncoderSettings(defaultEncoderSettings());
	});

	it('reads processing output config from canonical state selector', () => {
		const config = readOutputConfigForProcessing();

		expect(config).toMatchObject({
			outputPath: '/tmp/out',
			sampleRate: 'auto',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
		expect(config.encoderSettings).toEqual(defaultEncoderSettings());
	});

	it('publishes output draft mirror state for preset/template/directory', () => {
		updateOutputDirectory('/tmp/custom');
		updateNamingPreset('customTemplate');
		updateNamingTemplate('{author}/{title}.m4b');
		updateAbsIncludeYear(true);

		expect(appStore.outputDraft).toEqual({
			directory: '/tmp/custom',
			namingPreset: 'customTemplate',
			namingTemplate: '{author}/{title}.m4b',
			includeYear: true,
		});
	});

	it('defaults custom template when custom preset is selected without template text', () => {
		updateNamingPreset('customTemplate');
		updateNamingTemplate('   ');

		const config = readOutputConfigForProcessing();
		expect(config.outputNaming).toMatchObject({
			preset: 'customTemplate',
			customTemplate: '{author}/{title}',
		});
	});

	it('updates estimated size from shared state', () => {
		updateEncoderSettings({
			...defaultEncoderSettings(),
			bitrateKbps: 48,
			channels: 'mono',
		});
		updateEstimatedSize();
		const lowValue = document.getElementById('estimated-size')?.textContent ?? '';

		updateEncoderSettings({
			...defaultEncoderSettings(),
			bitrateKbps: 128,
			channels: 'stereo',
		});
		updateEstimatedSize();
		const highValue = document.getElementById('estimated-size')?.textContent ?? '';

		const lowMegabytes = Number.parseFloat(lowValue.replace(/[^\d.]+/g, ''));
		const highMegabytes = Number.parseFloat(highValue.replace(/[^\d.]+/g, ''));

		expect(lowMegabytes).toBeGreaterThan(0);
		expect(highMegabytes).toBeGreaterThan(lowMegabytes);
		expect(highMegabytes).toBeGreaterThan(lowMegabytes * 3);
	});
});
