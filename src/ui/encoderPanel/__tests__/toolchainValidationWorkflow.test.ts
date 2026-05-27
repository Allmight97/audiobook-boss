import { describe, expect, it, vi } from 'vitest';
import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import type { EncoderAvailability } from '../../../types/audio';
import { runtimeSettingsCapabilitiesFixture } from '../../../test/fixtures/runtimeSettingsCapabilities';
import {
	makeToolchainValidationWorkflowServicesLayer,
	toolchainValidationWorkflowExecution,
	type ToolchainValidationWorkflowServices,
} from '../toolchainValidationWorkflow';

function availability(overrides: Partial<EncoderAvailability> = {}): EncoderAvailability {
	return {
		fdkAvailable: true,
		aacAtAvailable: true,
		nativeAacAvailable: true,
		fdkSource: 'detected',
		autoEncoder: 'fdk_he_aac',
		detectedToolchainPath: '/opt/homebrew/bin/ffmpeg',
		overrideToolchainPath: undefined,
		activeToolchainPath: '/opt/homebrew/bin/ffmpeg',
		overrideInvalid: false,
		overrideError: undefined,
		statusMessage: 'FDK AAC detected and ready.',
		...overrides,
	};
}

function makeHarness(overrides: Partial<ToolchainValidationWorkflowServices> = {}) {
	const services: ToolchainValidationWorkflowServices = {
		readToolchainSettingsFromState: vi.fn(() => ({})),
		setEncoderSettingsCapabilities: vi.fn(),
		setExternalToolchainOverridePath: vi.fn(),
		syncAfterAvailabilityChange: vi.fn(),
		syncAfterToolchainPathChange: vi.fn(),
		openFile: vi.fn(async () => null),
		hydrateRuntimeSettingsCapabilities: vi.fn(async () =>
			runtimeSettingsCapabilitiesFixture({ encoder: { availability: availability() } }),
		),
		console: {
			log: vi.fn(),
			warn: vi.fn(),
		},
		...overrides,
	};

	return {
		services,
		layer: makeToolchainValidationWorkflowServicesLayer(services),
	};
}

describe('ToolchainValidationWorkflow', () => {
	it('loads initial encoder availability and synchronizes derived state', async () => {
		const loaded = availability();
		const capabilities = runtimeSettingsCapabilitiesFixture({ encoder: { availability: loaded } });
		const harness = makeHarness({
			hydrateRuntimeSettingsCapabilities: vi.fn(async () => capabilities),
		});

		await runAppEffect(
			toolchainValidationWorkflowExecution({ type: 'hydrateAvailability' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.hydrateRuntimeSettingsCapabilities).toHaveBeenCalledWith({});
		expect(harness.services.setEncoderSettingsCapabilities).toHaveBeenCalledWith(
			capabilities.encoder,
		);
		expect(harness.services.syncAfterAvailabilityChange).toHaveBeenCalledTimes(1);
	});

	it('maps availability load failure to unavailable state and still syncs UI state', async () => {
		const cause = new Error('toolchain probe failed');
		const harness = makeHarness({
			hydrateRuntimeSettingsCapabilities: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			toolchainValidationWorkflowExecution({ type: 'hydrateAvailability' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.setEncoderSettingsCapabilities).toHaveBeenCalledWith(null);
		expect(harness.services.console.warn).toHaveBeenCalledWith(
			'Failed to load runtime settings capabilities',
			cause,
		);
		expect(harness.services.syncAfterAvailabilityChange).toHaveBeenCalledTimes(1);
	});

	it('does nothing when toolchain browse is cancelled', async () => {
		const harness = makeHarness({
			openFile: vi.fn(async () => null),
		});

		await runAppEffect(
			toolchainValidationWorkflowExecution({ type: 'browseToolchain' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.setExternalToolchainOverridePath).not.toHaveBeenCalled();
		expect(harness.services.hydrateRuntimeSettingsCapabilities).not.toHaveBeenCalled();
	});

	it('stores browsed toolchain path and refreshes availability', async () => {
		const refreshed = availability({
			fdkSource: 'override',
			activeToolchainPath: '/custom/ffmpeg',
		});
		const capabilities = runtimeSettingsCapabilitiesFixture({
			encoder: { availability: refreshed },
		});
		const harness = makeHarness({
			openFile: vi.fn(async () => '/custom/ffmpeg'),
			hydrateRuntimeSettingsCapabilities: vi.fn(async () => capabilities),
		});

		await runAppEffect(
			toolchainValidationWorkflowExecution({ type: 'browseToolchain' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.setExternalToolchainOverridePath).toHaveBeenCalledWith(
			'/custom/ffmpeg',
		);
		expect(harness.services.syncAfterToolchainPathChange).toHaveBeenCalledTimes(1);
		expect(harness.services.hydrateRuntimeSettingsCapabilities).toHaveBeenCalledWith({});
		expect(harness.services.setEncoderSettingsCapabilities).toHaveBeenCalledWith(
			capabilities.encoder,
		);
	});

	it('clears override before refreshing availability', async () => {
		const harness = makeHarness();

		await runAppEffect(
			toolchainValidationWorkflowExecution({ type: 'clearOverride' }).pipe(
				Effect.provide(harness.layer),
			),
		);

		expect(harness.services.setExternalToolchainOverridePath).toHaveBeenCalledWith('');
		expect(harness.services.syncAfterToolchainPathChange).toHaveBeenCalledTimes(1);
		expect(harness.services.hydrateRuntimeSettingsCapabilities).toHaveBeenCalledWith({});
	});
});
