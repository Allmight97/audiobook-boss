import { createMemo, createRoot, createSignal, onCleanup } from 'solid-js';
import { bindAfterSettingsReset, hydrateConcurrencyAtom } from '../appSettings';
import { bindLookupInput, bindLookupMetadata } from '../metadataLookup';
import { bindMetadataInput } from '../metadataSession';
import { createMetadataOwner } from '../metadataSession/owner';
import { createOutputOwner } from '../outputPlan/owner';
import { bindProcessingInput, bindProcessingMetadata, seedProcessing } from '../processing';
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
		bindProcessingInput(input);
		bindProcessingMetadata(metadata);
		const remoteSource = createRemoteSourceOwner({ input });
		bindAfterSettingsReset((defaults) => {
			output.applyDefaults(defaults.outputDefaults);
			registry.set(hydrateConcurrencyAtom, {
				preference: defaults.maxConcurrentJobs,
			});
		});
		return { input, metadata, output, remoteSource };
	});
	return {
		input: runtime.input,
		metadata: runtime.metadata,
		output: runtime.output,
		remoteSource: runtime.remoteSource,
		registry,
		dispose(): void {
			runtime.remoteSource.reset();
			runtime.output.reset();
			runtime.metadata.reset();
			runtime.input.reset();
			bindMetadataInput(undefined);
			bindLookupInput(undefined);
			bindLookupMetadata(undefined);
			bindProcessingInput(undefined);
			bindProcessingMetadata(undefined);
			bindAfterSettingsReset(undefined);
			disposeWorkCenter();
			bindWorkOperationsRegistry(null);
			registry.dispose();
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
