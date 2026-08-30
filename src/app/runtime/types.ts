import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { SettingsOwner } from '../appSettings/owner';
import type { InputOwner } from '../inputSession/owner';
import type { MetadataLookupOwner } from '../metadataLookup/owner';
import type { MetadataOwner } from '../metadataSession/owner';
import type { OutputPlanOwner } from '../outputPlan/owner';
import type { ProcessingOwner } from '../processing/owner';
import type { RemoteSourceOwner } from '../remoteSource/owner';
import type { WorkOperationsOwner } from '../workOperations/owner';

export type RuntimeCapabilities = {
	readonly input?: InputCapability;
	readonly metadata?: MetadataCapability;
	readonly settings?: SettingsCapability;
};

export type AppRuntime = {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
	readonly lookup: MetadataLookupOwner;
	readonly output: OutputPlanOwner;
	readonly remoteSource: RemoteSourceOwner;
	readonly settings: SettingsOwner;
	readonly processing: ProcessingOwner;
	readonly workOperations: WorkOperationsOwner;
	dispose(): void;
};
