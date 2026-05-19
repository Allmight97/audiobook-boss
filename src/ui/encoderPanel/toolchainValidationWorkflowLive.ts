import { tauriClient } from '../../lib/tauri/client';
import {
	readToolchainSettingsFromState,
	setEncoderAvailability,
	setExternalToolchainOverridePath,
} from './state.svelte';
import { syncAfterStateChange, syncEncoderPanelAfterAvailabilityChange } from './logic';
import {
	makeToolchainValidationWorkflowServicesLayer,
	type ToolchainValidationWorkflowServices,
} from './toolchainValidationWorkflowServices';

export const liveToolchainValidationWorkflowServices: ToolchainValidationWorkflowServices = {
	readToolchainSettingsFromState,
	setEncoderAvailability,
	setExternalToolchainOverridePath,
	syncAfterAvailabilityChange: syncEncoderPanelAfterAvailabilityChange,
	syncAfterToolchainPathChange: syncAfterStateChange,
	openFile: tauriClient.openFile,
	listAvailableEncoders: tauriClient.listAvailableEncoders,
	refreshExternalToolchain: tauriClient.refreshExternalToolchain,
	console,
};

export const ToolchainValidationWorkflowLive = makeToolchainValidationWorkflowServicesLayer(
	liveToolchainValidationWorkflowServices,
);
