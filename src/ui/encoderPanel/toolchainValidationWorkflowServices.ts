import type { tauriClient } from '../../lib/tauri/client';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type {
	readToolchainSettingsFromState,
	setEncoderAvailability,
	setExternalToolchainOverridePath,
} from './state.svelte';

export type ToolchainHydrationMode = 'initial' | 'refresh';

export interface ToolchainValidationWorkflowServices {
	readToolchainSettingsFromState: typeof readToolchainSettingsFromState;
	setEncoderAvailability: typeof setEncoderAvailability;
	setExternalToolchainOverridePath: typeof setExternalToolchainOverridePath;
	syncAfterAvailabilityChange: () => void;
	syncAfterToolchainPathChange: () => void;
	openFile: typeof tauriClient.openFile;
	listAvailableEncoders: typeof tauriClient.listAvailableEncoders;
	refreshExternalToolchain: typeof tauriClient.refreshExternalToolchain;
	console: Pick<Console, 'log' | 'warn'>;
}

export type ToolchainValidationWorkflowAction =
	| { type: 'hydrateAvailability'; mode?: ToolchainHydrationMode }
	| { type: 'browseToolchain' }
	| { type: 'clearOverride' }
	| { type: 'commitOverride' }
	| { type: 'refresh' };

export type ToolchainValidationWorkflowServicesId =
	'EncoderPanel/ToolchainValidationWorkflowServices';
export type ToolchainValidationWorkflowLayer = AppLayer<ToolchainValidationWorkflowServicesId>;

export const ToolchainValidationWorkflowServicesTag = makeWorkflowServiceTag<
	ToolchainValidationWorkflowServicesId,
	ToolchainValidationWorkflowServices
>('EncoderPanel/ToolchainValidationWorkflowServices');

export function makeToolchainValidationWorkflowServicesLayer(
	services: ToolchainValidationWorkflowServices,
): ToolchainValidationWorkflowLayer {
	return makeWorkflowLayer(ToolchainValidationWorkflowServicesTag, services);
}
