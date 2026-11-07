import { createEffect, createMemo, createRoot } from 'solid-js';
import { createSettingsOwner } from '../appSettings';
import { createEncodingOwner } from '../encoding';
import { attachEngineLink } from '../engineLink';
import { createInputOwner } from '../inputSession';
import { createMetadataLookupOwner } from '../metadataLookup';
import { createMetadataOwner } from '../metadataSession';
import { createOutputOwner } from '../outputPlan';
import { createProcessingOwner } from '../processing';
import { createRemoteSourceOwner } from '../remoteSource';
import { createWorkOperationsOwner } from '../workOperations';
import type { AppRuntime, RuntimeCapabilities } from './types';

export type { AppRuntime } from './types';

/** Composes the owners once the engine has attached and sent its first snapshot. */
export async function createAppRuntime(
	capabilities: RuntimeCapabilities = {},
): Promise<AppRuntime> {
	const link = await attachEngineLink(capabilities.engine);
	return createRoot((disposeRoot) => {
		const input = createInputOwner({ link, capability: capabilities.input });
		const settings = createSettingsOwner({ link });
		const metadata = createMetadataOwner({ link, capability: capabilities.metadata });
		const encoding = createEncodingOwner({ link });
		const output = createOutputOwner({ link });
		const lookup = createMetadataLookupOwner({ link });
		const remoteSource = createRemoteSourceOwner({ link, ...capabilities.remoteSource });
		const processing = createProcessingOwner({ link, settings });
		const workOperations = createWorkOperationsOwner({ link });
		const settingsDialogOpen = createMemo(() => settings.dialog().isOpen);
		createEffect(settingsDialogOpen, (open) => {
			if (open) void remoteSource.indexerConnection.load();
		});
		let disposed = false;
		return {
			engine: {
				refusal: link.refusal,
				dismissRefusal: link.dismissRefusal,
				quitReminder: link.quitReminder,
			},
			input,
			metadata,
			lookup,
			encoding,
			output,
			remoteSource,
			settings,
			processing,
			workOperations,
			dispose(): void {
				if (disposed) return;
				disposed = true;
				workOperations.reset();
				processing.reset();
				remoteSource.reset();
				output.reset();
				lookup.reset();
				metadata.reset();
				input.reset();
				link.dispose();
				disposeRoot();
			},
		};
	});
}

export { AppRuntimeProvider, useAppRuntime } from './context';
