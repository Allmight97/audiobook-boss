import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { FileImportView } from './FileImportView';

function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
	return {
		maxConcurrentJobs: { mode: 'auto' },
		defaultAcquisitionLane: 'audible',
		encoderDefaults: {
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
				afterburner: true,
			},
			sampleRate: 'auto',
		},
		outputDefaults: {
			outputNaming: {
				preset: 'absDefault',
				includeYear: false,
			},
		},
		toolchain: {},
		startupBehavior: 'rememberLastState',
		...overrides,
	};
}

function fakeSettings(initial: Partial<AppSettings> = {}): SettingsCapability {
	let current = settingsFixture(initial);
	return {
		getAppSettings: vi.fn(async () => current),
		updateAppSettings: vi.fn(async (patch) => {
			current = settingsFixture({
				...current,
				defaultAcquisitionLane:
					patch.defaultAcquisitionLane ?? current.defaultAcquisitionLane ?? 'audible',
			});
			return current;
		}),
		resetAppSettings: vi.fn(async () => {
			current = settingsFixture();
			return current;
		}),
		openFile: vi.fn(async () => null),
		getMaxConcurrentJobs: vi.fn(async () => 4),
		setMaxConcurrentJobs: vi.fn(async (value) => value ?? 4),
		getRuntimeSettingsCapabilities: vi.fn(async () => runtimeSettingsCapabilitiesFixture()),
	};
}

describe('FileImportView import split button', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		document.body.innerHTML = '';
	});

	it('opens the default acquisition lane from settings on main click', async () => {
		runtime = createTestAppRuntime({
			settings: fakeSettings({ defaultAcquisitionLane: 'indexer' }),
		});
		await runtime.settings.hydrateAcquisitionPreferences();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		await fireEvent.click(screen.getByRole('button', { name: 'Import' }));

		expect(runtime.remoteSource.view().isOpen).toBe(true);
		expect(runtime.remoteSource.view().providerId).toBe('indexer');
	});

	it('opens Audible from the caret when default lane is Indexer', async () => {
		runtime = createTestAppRuntime({
			settings: fakeSettings({ defaultAcquisitionLane: 'indexer' }),
		});
		await runtime.settings.hydrateAcquisitionPreferences();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		await fireEvent.click(screen.getByRole('button', { name: 'Import' }));
		expect(runtime.remoteSource.view().providerId).toBe('indexer');
		runtime.remoteSource.close();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		await fireEvent.click(document.getElementById('import-split-caret') as Element);
		await fireEvent.click(screen.getByTestId('import-lane-audible'));

		expect(runtime.remoteSource.view().isOpen).toBe(true);
		expect(runtime.remoteSource.view().providerId).toBe('audible');
		expect(document.getElementById('import-split-caret')).toHaveAttribute('aria-expanded', 'false');
	});
});
