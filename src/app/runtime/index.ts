import { createMemo, createRoot, createSignal, onCleanup } from 'solid-js';
import { bindAfterSettingsReset } from '../appSettings';
import { createSettingsOwner } from '../appSettings/owner';
import { createMetadataLookupOwner } from '../metadataLookup/owner';
import { createMetadataOwner } from '../metadataSession/owner';
import { createOutputOwner } from '../outputPlan/owner';
import { createProcessingOwner } from '../processing/owner';
import { createInputOwner } from '../inputSession/owner';
import { createRemoteSourceOwner } from '../remoteSource/owner';
import { createWorkOperationsOwner } from '../workOperations/owner';
import { estimateKbpsFromRequest } from '../../ui/encoderPanel/requestConfig';
import { readEncodingRequestConfig, subscribeEncoderPanel } from '../../ui/encoderPanel/state';
import type { AppRuntime, RuntimeCapabilities } from './types';

export type { AppRuntime, RuntimeCapabilities } from './types';

export function createAppRuntime(capabilities: RuntimeCapabilities = {}): AppRuntime {
	let disposeRoot = (): void => {};
	const runtime = createRoot((dispose) => {
		disposeRoot = dispose;
		const selectionGate: { check?: () => Promise<boolean> } = {};
		const input = createInputOwner({
			capability: capabilities.input,
			beforeSelectionChange: () => selectionGate.check?.() ?? true,
		});
		const settings = createSettingsOwner({ capability: capabilities.settings });
		const processingHolder: { current?: ReturnType<typeof createProcessingOwner> } = {};
		const metadata = createMetadataOwner({
			input,
			capability: capabilities.metadata,
			isForegroundProcessing: () => processingHolder.current?.isProcessing() ?? false,
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
		const lookup = createMetadataLookupOwner({ input, metadata });
		const processing = createProcessingOwner({
			input,
			metadata,
			settings,
			encodingRequest,
		});
		processingHolder.current = processing;
		const remoteSource = createRemoteSourceOwner({ input });
		const workOperations = createWorkOperationsOwner();
		bindAfterSettingsReset((defaults) => {
			output.applyDefaults(defaults.outputDefaults);
			void settings.hydrateConcurrency({ preference: defaults.maxConcurrentJobs });
		});
		return {
			input,
			metadata,
			lookup,
			output,
			remoteSource,
			settings,
			processing,
			workOperations,
		};
	});
	return {
		...runtime,
		dispose(): void {
			runtime.workOperations.reset();
			runtime.processing.reset();
			runtime.remoteSource.reset();
			runtime.settings.reset();
			runtime.output.reset();
			runtime.lookup.reset();
			runtime.metadata.reset();
			runtime.input.reset();
			bindAfterSettingsReset(undefined);
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
