import { createMemo, createRoot, createSignal, onCleanup } from 'solid-js';
import { bindAfterSettingsReset } from '../appSettings';
import { createSettingsOwner } from '../appSettings/owner';
import { bindLookupInput, bindLookupMetadata } from '../metadataLookup';
import { bindMetadataInput } from '../metadataSession';
import { createMetadataOwner } from '../metadataSession/owner';
import { createOutputOwner } from '../outputPlan/owner';
import {
	bindProcessingInput,
	bindProcessingMetadata,
	bindProcessingSettings,
	seedProcessing,
} from '../processing';
import { getStatusView } from '../processing/view';
import { createInputOwner } from '../inputSession/owner';
import { createRemoteSourceOwner } from '../remoteSource/owner';
import { bindWorkOperationsRegistry, disposeWorkCenter } from '../workOperations';
import { estimateKbpsFromRequest } from '../../ui/encoderPanel/requestConfig';
import { readEncodingRequestConfig, subscribeEncoderPanel } from '../../ui/encoderPanel/state';
import { AtomRegistry } from './reactivity';
import type { AppRuntime, RuntimeCapabilities } from './types';

export type { AppRuntime, RuntimeCapabilities } from './types';

export function createAppRuntime(capabilities: RuntimeCapabilities = {}): AppRuntime {
	const registry = AtomRegistry.make();
	seedProcessing(registry);
	bindWorkOperationsRegistry(registry);
	let disposeRoot = (): void => {};
	const runtime = createRoot((dispose) => {
		disposeRoot = dispose;
		const selectionGate: { check?: () => Promise<boolean> } = {};
		const input = createInputOwner({
			capability: capabilities.input,
			beforeSelectionChange: () => selectionGate.check?.() ?? true,
		});
		const metadata = createMetadataOwner({
			input,
			capability: capabilities.metadata,
			isForegroundProcessing: () => getStatusView().isProcessing,
		});
		selectionGate.check = () => metadata.canChangeSelection();
		const [encodingRequest, setEncodingRequest] = createSignal(readEncodingRequestConfig());
		onCleanup(
			subscribeEncoderPanel(() => {
				setEncodingRequest(readEncodingRequestConfig());
			}),
		);
		const encodingEstimateKbps = createMemo(() => estimateKbpsFromRequest(encodingRequest()));
		const output = createOutputOwner({
			input,
			metadataView: metadata.view,
			encodingRequest,
			encodingEstimateKbps,
			onMetadataValidation: (validation) => metadata.applyDraftValidation(validation),
		});
		bindMetadataInput(input);
		bindLookupInput(input);
		bindLookupMetadata(metadata);
		const settings = createSettingsOwner({ capability: capabilities.settings });
		bindProcessingInput(input);
		bindProcessingMetadata(metadata);
		bindProcessingSettings(settings);
		const remoteSource = createRemoteSourceOwner({ input });
		bindAfterSettingsReset((defaults) => {
			output.applyDefaults(defaults.outputDefaults);
			void settings.hydrateConcurrency({ preference: defaults.maxConcurrentJobs });
		});
		return { input, metadata, output, remoteSource, settings };
	});
	return {
		input: runtime.input,
		metadata: runtime.metadata,
		output: runtime.output,
		remoteSource: runtime.remoteSource,
		settings: runtime.settings,
		registry,
		dispose(): void {
			runtime.remoteSource.reset();
			runtime.settings.reset();
			runtime.output.reset();
			runtime.metadata.reset();
			runtime.input.reset();
			bindMetadataInput(undefined);
			bindLookupInput(undefined);
			bindLookupMetadata(undefined);
			bindProcessingInput(undefined);
			bindProcessingMetadata(undefined);
			bindProcessingSettings(undefined);
			bindAfterSettingsReset(undefined);
			disposeWorkCenter();
			bindWorkOperationsRegistry(null);
			registry.dispose();
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
