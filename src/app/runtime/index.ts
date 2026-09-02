import { createRoot, runWithOwner } from 'solid-js';
import { createSettingsOwner } from '../appSettings';
import { createEncodingOwner } from '../encoding';
import { createInputOwner } from '../inputSession';
import { createMetadataLookupOwner } from '../metadataLookup';
import { createMetadataOwner } from '../metadataSession';
import { createOutputOwner } from '../outputPlan';
import { createProcessingOwner } from '../processing';
import { createRemoteSourceOwner } from '../remoteSource';
import { createWorkOperationsOwner } from '../workOperations';
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
			const encoding = createEncodingOwner({
				input,
				loadCapabilities: async () =>
					(await settings.capability().getRuntimeSettingsCapabilities()).encoder ?? null,
				persistDefaults: (defaults) => {
					void settings
						.capability()
						.updateAppSettings({ encoderDefaults: defaults })
						.catch((error: unknown) => {
							console.warn('Failed to persist encoder defaults:', error);
						});
				},
			});
			const output = createOutputOwner({
				input,
				metadataView: metadata.view,
				encoding,
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
				encoding,
				output,
				remoteSource,
			});
			processingHolder.current = processing;
			const workOperations = createWorkOperationsOwner({ remoteSource });
			settings.bindAfterReset((defaults) => {
				output.applyDefaults(defaults.outputDefaults);
				encoding.applyDefaults(defaults.encoderDefaults);
				void settings.hydrateConcurrency({ preference: defaults.maxConcurrentJobs });
			});
			return {
				input,
				metadata,
				lookup,
				encoding,
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
			runtime.encoding.reset();
			runtime.lookup.reset();
			runtime.metadata.reset();
			runtime.input.reset();
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
