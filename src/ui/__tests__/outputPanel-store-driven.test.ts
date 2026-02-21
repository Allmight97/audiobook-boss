import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings } from '../../types/audio';
import { updateEstimatedSize } from '../outputPanel/dom';
import {
	readOutputConfigForProcessing,
	updateAbsCompatible,
	updateAbsIncludeYear,
	updateEncoderSettings,
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
		updateAbsCompatible(true);
		updateAbsIncludeYear(false);
		updateSampleRate('auto');
		updateEncoderSettings(defaultEncoderSettings());
	});

	it('reads processing output config from canonical state selector', () => {
		const config = readOutputConfigForProcessing();

		expect(config).toMatchObject({
			outputPath: '/tmp/out',
			sampleRate: 'auto',
			outputNaming: { absCompatible: true, includeYear: false },
		});
		expect(config.encoderSettings).toEqual(defaultEncoderSettings());
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
