import { createMemo, createRoot, createSignal, onCleanup, runWithOwner } from 'solid-js';
import { createSettingsOwner } from '../appSettings';
import { createInputOwner } from '../inputSession';
import { createMetadataLookupOwner } from '../metadataLookup';
import { createMetadataOwner } from '../metadataSession';
import { createOutputOwner } from '../outputPlan';
import { createProcessingOwner } from '../processing';
import { createRemoteSourceOwner } from '../remoteSource';
import { createWorkOperationsOwner } from '../workOperations';
import {
	estimateKbpsFromRequest,
	readEncodingRequestConfig,
	subscribeEncoderPanel,
} from '../../ui/encoderPanel';
import type { AppRuntime, RuntimeCapabilities } from './types';

export type { AppRuntime, RuntimeCapabilities } from './types';

export function createAppRuntime(capabilities: RuntimeCapabilities = {}): AppRuntime {
	let disposeRoot = (): void => {};
	let disposed = false;
	const runtime = runWithOwner(null, () =>
		createRoot((dispose) => {
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
			const remoteSource = createRemoteSourceOwner({
				...capabilities.remoteSource,
				input,
			});
			const processing = createProcessingOwner({
				input,
				metadata,
				settings,
				encodingRequest,
				remoteSource,
			});
			processingHolder.current = processing;
			const workOperations = createWorkOperationsOwner({ remoteSource });
			settings.bindAfterReset((defaults) => {
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
		}),
	);
	return {
		...runtime,
		dispose(): void {
			if (disposed) {
				return;
			}
			disposed = true;
			runtime.workOperations.reset();
			runtime.processing.reset();
			runtime.remoteSource.reset();
			runtime.settings.reset();
			runtime.output.reset();
			runtime.lookup.reset();
			runtime.metadata.reset();
			runtime.input.reset();
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
