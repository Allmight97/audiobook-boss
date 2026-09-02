import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { SettingsOwner } from '../appSettings';
import type { InputOwner } from '../inputSession';
import type { MetadataLookupOwner } from '../metadataLookup';
import type { MetadataOwner } from '../metadataSession';
import type { EncodingOwner } from '../encoding';
import type { OutputPlanOwner } from '../outputPlan';
import type { ProcessingOwner } from '../processing';
import type { RemoteSourceOwner, RemoteSourceOwnerDeps } from '../remoteSource';
import type { WorkOperationsOwner } from '../workOperations';

export type RuntimeCapabilities = {
	readonly input?: InputCapability;
	readonly metadata?: MetadataCapability;
	readonly settings?: SettingsCapability;
	readonly remoteSource?: Omit<RemoteSourceOwnerDeps, 'input'>;
};

export type AppRuntime = {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
	readonly lookup: MetadataLookupOwner;
	readonly encoding: EncodingOwner;
	readonly output: OutputPlanOwner;
	readonly remoteSource: RemoteSourceOwner;
	readonly settings: SettingsOwner;
	readonly processing: ProcessingOwner;
	readonly workOperations: WorkOperationsOwner;
	dispose(): void;
};
