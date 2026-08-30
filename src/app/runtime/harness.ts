import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import { resetProductionSettingsDialog } from '../appSettings/dialog';
import { resetMetadataLookupState } from '../metadataLookup';
import { clearMetadataLookupCoverPreviewCache } from '../metadataLookup/coverPreview';
import { resetCollisionDialog } from '../outputPlan';
import { clearMetadataSession } from '../metadataSession/cache';
import { createAppRuntime, type AppRuntime } from './index';

export function createTestAppRuntime(
	options: {
		readonly input?: InputCapability;
		readonly metadata?: MetadataCapability;
		readonly settings?: SettingsCapability;
	} = {},
): AppRuntime {
	const runtime = createAppRuntime(options);
	clearMetadataSession();
	resetMetadataLookupState();
	clearMetadataLookupCoverPreviewCache();
	resetProductionSettingsDialog();
	resetCollisionDialog();
	return runtime;
}
