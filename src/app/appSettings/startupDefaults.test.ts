import { describe, expect, it } from 'vitest';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { AppSettings, PinnedDefaults } from '../../types/appSettings';
import { resolveStartupDefaults } from './startupDefaults';

function notCalled(): never {
	throw new Error('resolveStartupDefaults may only read app settings');
}

function capabilityFor(settings: AppSettings): SettingsCapability {
	return {
		getAppSettings: () => Promise.resolve(settings),
		updateAppSettings: notCalled,
		resetAppSettings: notCalled,
		openFile: notCalled,
		getMaxConcurrentJobs: notCalled,
		setMaxConcurrentJobs: notCalled,
		getRuntimeSettingsCapabilities: notCalled,
	};
}

const lastUsed: AppSettings = {
	maxConcurrentJobs: { mode: 'fixed', value: 3 },
	encoderDefaults: {
		settings: {
			encoderType: 'native_aac',
			bitrateKbps: 64,
			bitrateMode: { mode: 'cbr' },
			channels: 'mono',
			afterburner: false,
		},
		sampleRate: { explicit: 44100 },
	},
	outputDefaults: {
		outputDirectory: '/books/last-used',
		outputNaming: { preset: 'absDefault', includeYear: false },
	},
	toolchain: {},
	startupBehavior: 'rememberLastState',
};

const pinned: PinnedDefaults = {
	maxConcurrentJobs: { mode: 'auto' },
	encoderDefaults: {
		settings: {
			encoderType: 'fdk_he_aac',
			bitrateKbps: 128,
			bitrateMode: { mode: 'cbr' },
			channels: 'stereo',
			afterburner: true,
		},
		sampleRate: { explicit: 48000 },
	},
	outputDefaults: {
		outputDirectory: '/books/pinned',
		outputNaming: { preset: 'customTemplate', includeYear: true, customTemplate: '{title}' },
	},
};

describe('resolveStartupDefaults', () => {
	it('restores last-used values in rememberLastState', async () => {
		const resolved = await resolveStartupDefaults(capabilityFor(lastUsed));

		expect(resolved.outputDefaults.outputDirectory).toBe('/books/last-used');
		expect(resolved.maxConcurrentJobs).toEqual({ mode: 'fixed', value: 3 });
	});

	it('restores the pinned snapshot when pinned startup behavior has a pin', async () => {
		const resolved = await resolveStartupDefaults(
			capabilityFor({ ...lastUsed, startupBehavior: 'pinnedDefaults', pinnedDefaults: pinned }),
		);

		expect(resolved.outputDefaults.outputDirectory).toBe('/books/pinned');
		expect(resolved.encoderDefaults.settings.encoderType).toBe('fdk_he_aac');
		expect(resolved.maxConcurrentJobs).toEqual({ mode: 'auto' });
	});

	it('falls back to last-used values when pinned mode has nothing pinned', async () => {
		const resolved = await resolveStartupDefaults(
			capabilityFor({ ...lastUsed, startupBehavior: 'pinnedDefaults' }),
		);

		expect(resolved.outputDefaults.outputDirectory).toBe('/books/last-used');
		expect(resolved.maxConcurrentJobs).toEqual({ mode: 'fixed', value: 3 });
	});

	it('ignores an existing pin while startup behavior is rememberLastState', async () => {
		const resolved = await resolveStartupDefaults(
			capabilityFor({ ...lastUsed, pinnedDefaults: pinned }),
		);

		expect(resolved.outputDefaults.outputDirectory).toBe('/books/last-used');
		expect(resolved.encoderDefaults.settings.encoderType).toBe('native_aac');
	});
});
