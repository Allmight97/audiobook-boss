import { createRoot, runWithOwner } from 'solid-js';
import { createSettingsOwner } from '../appSettings';
import { createEncodingOwner } from '../encoding';
import { createEngineLink } from '../engineLink';
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
			const link = createEngineLink(capabilities.engine);
			const input = createInputOwner({
				link,
				capability: capabilities.input,
				audioDefaults: () => encoding.audioRequest(),
				beforeImport: () => initialize(),
			});
			const settings = createSettingsOwner({ link, capability: capabilities.settings });
			const metadata = createMetadataOwner({ link, capability: capabilities.metadata });
			const encoding = createEncodingOwner({
				input,
				loadCapabilities: async () =>
					(await settings.capability().getRuntimeSettingsCapabilities()).encoder ?? null,
				persistDefaults: settings.rememberEncoderDefaults,
			});
			const output = createOutputOwner({
				input,
				metadataView: metadata.view,
				encoding,
				persistDefaults: settings.rememberOutputDefaults,
			});
			const lookup = createMetadataLookupOwner({ link, metadata });
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
			const workOperations = createWorkOperationsOwner({ remoteSource });
			let startup: Promise<void> | undefined;
			function initialize(): Promise<void> {
				if (startup) return startup;
				const initialOutput = JSON.stringify(output.readDefaults());
				startup = settings
					.loadStartupDefaults()
					.then((defaults) => {
						if (disposed) return;
						encoding.hydrateDefaults(defaults.encoderDefaults);
						if (JSON.stringify(output.readDefaults()) === initialOutput)
							output.applyDefaults(defaults.outputDefaults);
					})
					.catch((error: unknown) => {
						startup = undefined;
						throw error;
					});
				return startup;
			}
			settings.bindAfterReset((defaults) => {
				output.applyDefaults(defaults.outputDefaults);
				encoding.applyDefaults(defaults.encoderDefaults);
			});
			return {
				link,
				initialize,
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
	const { link, ...owners } = runtime;
	return {
		...owners,
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
			link.dispose();
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
