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
			const input = createInputOwner({ link, capability: capabilities.input });
			const settings = createSettingsOwner({ link });
			const metadata = createMetadataOwner({ link, capability: capabilities.metadata });
			const encoding = createEncodingOwner({ link, input });
			const output = createOutputOwner({ link });
			const lookup = createMetadataLookupOwner({ link, metadata });
			const remoteSource = createRemoteSourceOwner({
				...capabilities.remoteSource,
				input,
			});
			const processing = createProcessingOwner({
				link,
				input,
				settings,
				output,
				remoteSource,
			});
			const workOperations = createWorkOperationsOwner({ remoteSource });
			/** Resolves once the engine's session and settings have arrived. */
			function initialize(): Promise<void> {
				return link.ready();
			}
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
