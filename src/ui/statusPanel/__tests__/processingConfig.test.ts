import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings } from '../../../types/audio';
import { readProcessingRequestConfig } from '../processingConfig';

const context = vi.hoisted(() => ({
	readEncodingRequestConfigMock: vi.fn(),
	readOutputRequestConfigMock: vi.fn(),
}));

vi.mock('../../encoderPanel', () => ({
	readEncodingRequestConfig: context.readEncodingRequestConfigMock,
}));

vi.mock('../../outputPanel', () => ({
	readOutputRequestConfig: context.readOutputRequestConfigMock,
}));

describe('processing config composition', () => {
	beforeEach(() => {
		context.readEncodingRequestConfigMock.mockReset();
		context.readOutputRequestConfigMock.mockReset();
		context.readEncodingRequestConfigMock.mockReturnValue({
			encoderSettings: defaultEncoderSettings(),
			toolchainSettings: { overridePath: '/opt/ffmpeg' },
			sampleRate: 'auto',
		});
		context.readOutputRequestConfigMock.mockReturnValue({
			outputDirectory: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
	});

	it('composes process config from encoder and output public strips', () => {
		const config = readProcessingRequestConfig();

		expect(config).toEqual({
			encoderSettings: defaultEncoderSettings(),
			toolchainSettings: { overridePath: '/opt/ffmpeg' },
			sampleRate: 'auto',
			outputDirectory: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
		expect(context.readEncodingRequestConfigMock).toHaveBeenCalledTimes(1);
		expect(context.readOutputRequestConfigMock).toHaveBeenCalledTimes(1);
	});
});
