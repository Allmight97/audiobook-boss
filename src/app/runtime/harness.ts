import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import { settingsCapabilityAtom, concurrencyViewAtom } from '../appSettings/concurrency';
import { resetProductionSettingsDialog } from '../appSettings/dialog';
import { resetMetadataLookupState } from '../metadataLookup';
import { clearMetadataLookupCoverPreviewCache } from '../metadataLookup/coverPreview';
import { resetCollisionDialog, seedOutputPlan } from '../outputPlan';
import { metadataCapabilityAtom, metadataEditorAtom } from '../metadataSession/atoms';
import { clearMetadataSession } from '../metadataSession/cache';
import { createEmptyFormState } from '../metadataSession/fields';
import { createEmptyCoverUiState } from '../metadataSession/cover';
import { createAppRuntime, type AppRuntime } from './index';

function emptyMetadataEditor() {
	return {
		form: createEmptyFormState(),
		cover: createEmptyCoverUiState(),
		saveInProgress: false,
		formRevision: 0,
		coverRevision: 0,
		focusedFieldId: null,
		selectionKey: '',
		boundFiles: [],
		hydrateRequestId: 0,
		autoCoverRequestId: 0,
		statusMessage: '',
	};
}

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
	seedOutputPlan(runtime.registry);
	runtime.registry.set(concurrencyViewAtom, {
		selection: 'auto',
		effective: null,
		effectiveLabel: '',
		controlsEnabled: true,
		allowAuto: true,
		fixedOptions: [],
	});
	runtime.registry.set(metadataEditorAtom, emptyMetadataEditor());
	if (options.metadata) {
		runtime.registry.set(metadataCapabilityAtom, options.metadata);
	}
	if (options.settings) {
		runtime.registry.set(settingsCapabilityAtom, options.settings);
	}
	return runtime;
}
