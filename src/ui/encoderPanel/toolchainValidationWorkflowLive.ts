import {
	readToolchainSettingsFromState,
	setEncoderSettingsCapabilities,
	setExternalToolchainOverridePath,
} from './state.svelte';
import { tauriClient } from '../../lib/tauri/client';
import { hydrateRuntimeSettingsCapabilities } from '../runtimeSettingsCapabilities.svelte';
import { syncAfterStateChange, syncEncoderPanelAfterAvailabilityChange } from './logic';
import {
	makeToolchainValidationWorkflowServicesLayer,
	type ToolchainValidationWorkflowServices,
} from './toolchainValidationWorkflowServices';

export const liveToolchainValidationWorkflowServices: ToolchainValidationWorkflowServices = {
	readToolchainSettingsFromState,
	setEncoderSettingsCapabilities,
	setExternalToolchainOverridePath,
	syncAfterAvailabilityChange: syncEncoderPanelAfterAvailabilityChange,
	syncAfterToolchainPathChange: syncAfterStateChange,
	openFile: tauriClient.openFile,
	hydrateRuntimeSettingsCapabilities,
	console,
};

export const ToolchainValidationWorkflowLive = makeToolchainValidationWorkflowServicesLayer(
	liveToolchainValidationWorkflowServices,
);
