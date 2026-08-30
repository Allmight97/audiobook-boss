import { createRoot } from 'solid-js';
import {
	bindAfterSettingsReset,
	hydrateConcurrencyAtom,
} from '../appSettings';
import { bindLookupInput } from '../metadataLookup';
import { bindMetadataInput } from '../metadataSession';
import { applyOutputDefaultsFromSettings } from '../outputPlan';
import { bindProcessingInput, seedProcessing } from '../processing';
import { createInputOwner } from '../inputSession/owner';
import { createRemoteSourceOwner } from '../remoteSource/owner';
import { bindWorkOperationsRegistry, disposeWorkCenter } from '../workOperations';
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
		const input = createInputOwner({ capability: capabilities.input });
		bindMetadataInput(input);
		bindLookupInput(input);
		bindProcessingInput(input);
		const remoteSource = createRemoteSourceOwner({ input });
		bindAfterSettingsReset((defaults) => {
			registry.set(applyOutputDefaultsFromSettings, defaults.outputDefaults);
			registry.set(hydrateConcurrencyAtom, {
				preference: defaults.maxConcurrentJobs,
			});
		});
		return { input, remoteSource };
	});
	return {
		input: runtime.input,
		remoteSource: runtime.remoteSource,
		registry,
		dispose(): void {
			runtime.remoteSource.reset();
			runtime.input.reset();
			bindMetadataInput(undefined);
			bindLookupInput(undefined);
			bindProcessingInput(undefined);
			bindAfterSettingsReset(undefined);
			disposeWorkCenter();
			bindWorkOperationsRegistry(null);
			registry.dispose();
			disposeRoot();
		},
	};
}

export { AppRuntimeProvider, useAppRuntime } from './context';
