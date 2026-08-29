import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import { settingsCapabilityAtom, concurrencyViewAtom } from '../appSettings/concurrency';
import { resetProductionSettingsDialog } from '../appSettings/dialog';
import { inputCapabilityAtom, inputSessionAtom } from '../inputSession/atoms';
import { emptyInputSession } from '../inputSession/types';
import { resetMetadataLookupState } from '../metadataLookup';
import { lookupViewAtom } from '../metadataLookup/atoms';
import { createMetadataLookupState } from '../metadataLookup/state';
import { clearMetadataLookupCoverPreviewCache } from '../metadataLookup/coverPreview';
import { resetCollisionDialog, seedOutputPlan } from '../outputPlan';
import { seedProcessing } from '../processing';
import { bindWorkOperationsRegistry, disposeWorkCenter } from '../workOperations';
import { encodingRequestConfigAtom } from '../../ui/encoderPanel/requestConfig';
import { readEncodingRequestConfig, subscribeEncoderPanel } from '../../ui/encoderPanel/state.svelte';
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
	const runtime = createAppRuntime();
	runtime.registry.set(inputSessionAtom, emptyInputSession());
	runtime.registry.set(metadataEditorAtom, emptyMetadataEditor());
	clearMetadataSession();
	resetMetadataLookupState();
	clearMetadataLookupCoverPreviewCache();
	resetProductionSettingsDialog();
	resetCollisionDialog();
	seedOutputPlan(runtime.registry);
	seedProcessing(runtime.registry);
	bindWorkOperationsRegistry(runtime.registry);
	runtime.registry.set(encodingRequestConfigAtom, readEncodingRequestConfig());
	const unsubscribeEncoder = subscribeEncoderPanel(() => {
		runtime.registry.set(encodingRequestConfigAtom, readEncodingRequestConfig());
	});
	runtime.registry.set(lookupViewAtom, createMetadataLookupState());
	runtime.registry.set(concurrencyViewAtom, {
		selection: 'auto',
		effective: null,
		effectiveLabel: '',
		controlsEnabled: true,
		allowAuto: true,
		fixedOptions: [],
	});
	if (options.input) {
		runtime.registry.set(inputCapabilityAtom, options.input);
	}
	if (options.metadata) {
		runtime.registry.set(metadataCapabilityAtom, options.metadata);
	}
	if (options.settings) {
		runtime.registry.set(settingsCapabilityAtom, options.settings);
	}
	const dispose = runtime.dispose.bind(runtime);
	return {
		...runtime,
		dispose(): void {
			unsubscribeEncoder();
			disposeWorkCenter();
			bindWorkOperationsRegistry(null);
			dispose();
		},
	};
}
