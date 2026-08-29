import type { OutputDefaults } from '../../types/appSettings';
import type { OutputRequestConfig } from '../../types/audio';
import { persistOutputDefaults } from '../../ui/appSettings';
import {
	encodingEstimateBitrateKbpsAtom,
	encodingRequestConfigAtom,
} from '../../ui/encoderPanel/requestConfig';
import { Effect } from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import { Atom, AtomRegistry } from '../runtime/reactivity';
import { inputViewAtom } from '../inputSession';
import { metadataViewAtom } from '../metadataSession';
import { formatEstimatedSizeText } from './estimate';
import { previewDraftFromMetadataView, sourcePathFromInput } from './previewDraft';
import {
	emptyOutputPlan,
	namingHintText,
	outputNamingFromPlan,
	EMPTY_PREVIEW_TEXT,
	EMPTY_PREVIEW_TITLE,
	type OutputNamingPreset,
	type OutputPlanState,
	type OutputView,
} from './types';
import {
	makeOutputPlanWorkflowServicesLayer,
	outputPathPreviewBody,
	showOutputError,
	updateMetadataIntentWarnings,
} from './workflow';

const TEMPLATE_PREVIEW_DEBOUNCE_MS = 150;

let planSnapshot: OutputPlanState = emptyOutputPlan();
let templatePreviewTimer: ReturnType<typeof setTimeout> | null = null;

export const outputDirectoryAtom = Atom.make('').pipe(Atom.keepAlive);
export const namingPresetAtom = Atom.make<OutputNamingPreset>('absDefault').pipe(Atom.keepAlive);
export const namingTemplateAtom = Atom.make('').pipe(Atom.keepAlive);
export const absIncludeYearAtom = Atom.make(false).pipe(Atom.keepAlive);
export const committedTemplateAtom = Atom.make('').pipe(Atom.keepAlive);
export const outputPreviewTextAtom = Atom.make(EMPTY_PREVIEW_TEXT).pipe(Atom.keepAlive);
export const outputPreviewTitleAtom = Atom.make(EMPTY_PREVIEW_TITLE).pipe(Atom.keepAlive);

export const estimatedSizeTextAtom = Atom.make((get): string => {
	const input = get(inputViewAtom);
	const request = get(encodingRequestConfigAtom);
	return formatEstimatedSizeText(input.hasFiles, input.totalDurationSeconds, {
		bitrateKbps: get(encodingEstimateBitrateKbpsAtom),
		channels: request.encoderSettings.channels,
	});
}).pipe(Atom.keepAlive);

export const outputViewAtom = Atom.make((get): OutputView => {
	const outputDirectory = get(outputDirectoryAtom);
	const namingPreset = get(namingPresetAtom);
	const namingTemplate = get(namingTemplateAtom);
	const absIncludeYear = get(absIncludeYearAtom);
	return {
		outputDirectory,
		namingPreset,
		namingTemplate,
		absIncludeYear,
		previewText: get(outputPreviewTextAtom),
		previewTitle: get(outputPreviewTitleAtom),
		absHintText: namingHintText(namingPreset, absIncludeYear),
		absHintHidden: namingPreset !== 'absDefault',
		templateRowHidden: namingPreset !== 'customTemplate',
		displayDirectory: outputDirectory || EMPTY_PREVIEW_TEXT,
		estimatedSizeText: get(estimatedSizeTextAtom),
	};
}).pipe(Atom.keepAlive);

export const outputPathPreviewAtom = Atom.make((get) => {
	const outputDirectory = get(outputDirectoryAtom);
	const namingPreset = get(namingPresetAtom);
	const absIncludeYear = get(absIncludeYearAtom);
	const previewTemplate = get(committedTemplateAtom);
	const input = get(inputViewAtom);
	const metadata = get(metadataViewAtom);
	const requestId = planSnapshot.latestPreviewRequestId + 1;
	planSnapshot = { ...planSnapshot, latestPreviewRequestId: requestId };

	const context = {
		outputDirectory,
		sourcePath: sourcePathFromInput(input),
		outputNaming: outputNamingFromPlan({
			...planSnapshot,
			outputDirectory,
			namingPreset,
			previewTemplate,
			absIncludeYear,
		}),
		metadataDraft: previewDraftFromMetadataView(metadata),
	};

	const layer = makeOutputPlanWorkflowServicesLayer({
		updateMetadataIntentWarnings,
		beginOutputPreviewRequest: () => requestId,
		isLatestOutputPreviewRequest: (id) => id === planSnapshot.latestPreviewRequestId,
		setOutputPreview: (text, title = text) => {
			get.set(outputPreviewTextAtom, text);
			get.set(outputPreviewTitleAtom, title);
			planSnapshot = { ...planSnapshot, previewText: text, previewTitle: title };
		},
		showOutputError,
		previewOutputPath: tauriClient.previewOutputPath,
		preflightProcessingPlan: tauriClient.preflightProcessingPlan,
		openCollisionDialog: () => Promise.resolve(null),
		console,
	});

	return outputPathPreviewBody('final', context, requestId).pipe(Effect.provide(layer));
}).pipe(Atom.keepAlive);

function persistPlan(): void {
	void persistOutputDefaults(readOutputDefaultsFromState());
}

function syncSnapshot(partial: Partial<OutputPlanState>): void {
	planSnapshot = { ...planSnapshot, ...partial };
}

export const applyOutputDefaultsFromSettings = Atom.fnSync((defaults: OutputDefaults, get) => {
	clearTemplatePreviewTimer();
	const outputDirectory = defaults.outputDirectory ?? '';
	const namingPreset = defaults.outputNaming.preset;
	const namingTemplate = defaults.outputNaming.customTemplate ?? '';
	const absIncludeYear = defaults.outputNaming.includeYear;
	get.set(outputDirectoryAtom, outputDirectory);
	get.set(namingPresetAtom, namingPreset);
	get.set(namingTemplateAtom, namingTemplate);
	get.set(committedTemplateAtom, namingTemplate);
	get.set(absIncludeYearAtom, absIncludeYear);
	syncSnapshot({
		outputDirectory,
		namingPreset,
		namingTemplate,
		previewTemplate: namingTemplate,
		absIncludeYear,
	});
	return defaults;
}).pipe(Atom.keepAlive);

export const browseOutputDirectoryAtom = Atom.fn((_: undefined, get) => {
	return Effect.tryPromise({
		try: () => tauriClient.openDirectory({ title: 'Select Output Directory' }),
		catch: (cause) => cause,
	}).pipe(
		Effect.match({
			onFailure: (cause) => {
				console.error('Error selecting directory:', cause);
				showOutputError('Failed to select directory');
				return null;
			},
			onSuccess: (selectedPath) => {
				if (!selectedPath) {
					return null;
				}
				clearTemplatePreviewTimer();
				get.set(outputDirectoryAtom, selectedPath);
				syncSnapshot({ outputDirectory: selectedPath });
				persistPlan();
				return selectedPath;
			},
		}),
	);
}).pipe(Atom.keepAlive);

export const selectNamingPresetAtom = Atom.fnSync((value: string, get) => {
	clearTemplatePreviewTimer();
	const namingPreset: OutputNamingPreset =
		value === 'customTemplate' ? 'customTemplate' : 'absDefault';
	get.set(namingPresetAtom, namingPreset);
	syncSnapshot({ namingPreset });
	persistPlan();
	return namingPreset;
}).pipe(Atom.keepAlive);

export const setAbsIncludeYearAtom = Atom.fnSync((absIncludeYear: boolean, get) => {
	clearTemplatePreviewTimer();
	get.set(absIncludeYearAtom, absIncludeYear);
	syncSnapshot({ absIncludeYear });
	persistPlan();
	return absIncludeYear;
}).pipe(Atom.keepAlive);

export const editNamingTemplateAtom = Atom.fnSync((namingTemplate: string, get) => {
	get.set(namingTemplateAtom, namingTemplate);
	syncSnapshot({ namingTemplate });
	scheduleCommittedTemplate((atom, value) => {
		get.set(atom, value);
	});
	return namingTemplate;
}).pipe(Atom.keepAlive);

export function readOutputDefaultsFromState(): OutputDefaults {
	return {
		outputDirectory: planSnapshot.outputDirectory || undefined,
		outputNaming: outputNamingFromPlan(planSnapshot),
	};
}

export function readOutputRequestConfig(): OutputRequestConfig {
	if (!planSnapshot.outputDirectory) {
		throw new Error('Output directory not selected');
	}
	return {
		outputDirectory: planSnapshot.outputDirectory,
		outputNaming: outputNamingFromPlan(planSnapshot),
	};
}

export function resetOutputPlan(): void {
	clearTemplatePreviewTimer();
	planSnapshot = emptyOutputPlan();
}

export function seedOutputPlan(registry: AtomRegistry.AtomRegistry): void {
	clearTemplatePreviewTimer();
	const empty = emptyOutputPlan();
	planSnapshot = empty;
	registry.set(outputDirectoryAtom, empty.outputDirectory);
	registry.set(namingPresetAtom, empty.namingPreset);
	registry.set(namingTemplateAtom, empty.namingTemplate);
	registry.set(committedTemplateAtom, empty.previewTemplate);
	registry.set(absIncludeYearAtom, empty.absIncludeYear);
	registry.set(outputPreviewTextAtom, empty.previewText);
	registry.set(outputPreviewTitleAtom, empty.previewTitle);
}

export function resetOutputPlanTimers(): void {
	clearTemplatePreviewTimer();
}

function clearTemplatePreviewTimer(): void {
	if (templatePreviewTimer) {
		clearTimeout(templatePreviewTimer);
		templatePreviewTimer = null;
	}
}

function scheduleCommittedTemplate(
	set: (atom: typeof committedTemplateAtom, value: string) => void,
): void {
	clearTemplatePreviewTimer();
	templatePreviewTimer = setTimeout(() => {
		templatePreviewTimer = null;
		set(committedTemplateAtom, planSnapshot.namingTemplate);
		syncSnapshot({ previewTemplate: planSnapshot.namingTemplate });
		persistPlan();
	}, TEMPLATE_PREVIEW_DEBOUNCE_MS);
}
