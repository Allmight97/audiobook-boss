import {
	batch,
	createEffect,
	createMemo,
	createSignal,
	onCleanup,
	untrack,
	type Accessor,
} from 'solid-js';
import type { OutputDefaults } from '../../types/appSettings';
import type {
	CollisionPolicy,
	EncodingRequestConfig,
	OutputNamingConfig,
	OutputRequestConfig,
	ProcessingPreflightPlan,
} from '../../types/audio';
import { persistOutputDefaults } from '../../ui/appSettings';
import { Effect, runAppEffect } from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import type { InputOwner } from '../inputSession';
import type { MetadataDraftValidation, MetadataView } from '../metadataSession';
import { createCollisionReview, type CollisionView } from './collision';
import { formatEstimatedSizeText } from './estimate';
import { bindOutputOwner, boundOutputOwner } from './bind';
import { previewDraftFromMetadataView, sourcePathFromInput } from './previewDraft';
import {
	emptyOutputPlan,
	namingHintText,
	outputNamingFromPlan,
	EMPTY_PREVIEW_TEXT,
	type OutputPlanState,
	type OutputView,
} from './types';
import {
	makeOutputPlanWorkflowServicesLayer,
	outputPathPreviewBody,
	showOutputError,
	updateMetadataIntentWarnings as applyMetadataIntentWarnings,
} from './workflow';

const TEMPLATE_PREVIEW_DEBOUNCE_MS = 150;

export type OutputPlanOwner = {
	readonly view: Accessor<OutputView>;
	readonly estimatedSizeText: Accessor<string>;
	readonly collision: Accessor<CollisionView>;
	applyDefaults(defaults: OutputDefaults): void;
	browseDirectory(): Promise<void>;
	selectNamingPreset(value: string): void;
	setAbsIncludeYear(value: boolean): void;
	editNamingTemplate(value: string): void;
	openCollisionReview(plan: ProcessingPreflightPlan): Promise<CollisionPolicy | null>;
	chooseCollisionPolicy(policy: CollisionPolicy): void;
	cancelCollisionReview(): void;
	readRequestConfig(): OutputRequestConfig;
	readDefaults(): OutputDefaults;
	reset(): void;
};

export type OutputOwnerDeps = {
	readonly input: InputOwner;
	readonly metadataView: Accessor<MetadataView>;
	readonly encodingRequest: Accessor<EncodingRequestConfig>;
	readonly encodingEstimateKbps: Accessor<number>;
	readonly onMetadataValidation?: (validation: MetadataDraftValidation) => void;
};

export function createOutputOwner(deps: OutputOwnerDeps): OutputPlanOwner {
	const empty = emptyOutputPlan();
	const [outputDirectory, setOutputDirectory] = createSignal(empty.outputDirectory);
	const [namingPreset, setNamingPreset] = createSignal(empty.namingPreset);
	const [namingTemplate, setNamingTemplate] = createSignal(empty.namingTemplate);
	const [previewTemplate, setPreviewTemplate] = createSignal(empty.previewTemplate);
	const [absIncludeYear, setAbsIncludeYearSignal] = createSignal(empty.absIncludeYear);
	const [previewText, setPreviewText] = createSignal(empty.previewText);
	const [previewTitle, setPreviewTitle] = createSignal(empty.previewTitle);
	let latestPreviewRequestId = empty.latestPreviewRequestId;
	let templatePreviewTimer: ReturnType<typeof setTimeout> | null = null;
	const collisionReview = createCollisionReview();

	const estimatedSizeText = createMemo(() => {
		const input = deps.input.view();
		const request = deps.encodingRequest();
		return formatEstimatedSizeText(input.hasFiles, input.totalDurationSeconds, {
			bitrateKbps: deps.encodingEstimateKbps(),
			channels: request.encoderSettings.channels,
		});
	});

	const view = createMemo((): OutputView => {
		const directory = outputDirectory();
		const preset = namingPreset();
		const year = absIncludeYear();
		return {
			outputDirectory: directory,
			namingPreset: preset,
			namingTemplate: namingTemplate(),
			absIncludeYear: year,
			previewText: previewText(),
			previewTitle: previewTitle(),
			absHintText: namingHintText(preset, year),
			absHintHidden: preset !== 'absDefault',
			templateRowHidden: preset !== 'customTemplate',
			displayDirectory: directory || EMPTY_PREVIEW_TEXT,
			estimatedSizeText: estimatedSizeText(),
		};
	});

	function plan(): OutputPlanState {
		return {
			outputDirectory: outputDirectory(),
			namingPreset: namingPreset(),
			namingTemplate: namingTemplate(),
			previewTemplate: previewTemplate(),
			absIncludeYear: absIncludeYear(),
			previewText: previewText(),
			previewTitle: previewTitle(),
			latestPreviewRequestId,
		};
	}

	function persistPlan(): void {
		void persistOutputDefaults({
			outputDirectory: outputDirectory() || undefined,
			outputNaming: outputNamingFromPlan(plan()),
		});
	}

	function outputNamingForSubmit(): OutputNamingConfig {
		const state = plan();
		return outputNamingFromPlan({ ...state, previewTemplate: state.namingTemplate });
	}

	function clearTemplatePreviewTimer(): void {
		if (templatePreviewTimer) {
			clearTimeout(templatePreviewTimer);
			templatePreviewTimer = null;
		}
	}

	function scheduleCommittedTemplate(): void {
		clearTemplatePreviewTimer();
		templatePreviewTimer = setTimeout(() => {
			templatePreviewTimer = null;
			setPreviewTemplate(namingTemplate());
			persistPlan();
		}, TEMPLATE_PREVIEW_DEBOUNCE_MS);
	}

	const metadataDraftKey = createMemo(() => {
		const draft = previewDraftFromMetadataView(deps.metadataView());
		return [
			draft.title,
			draft.album,
			draft.artist,
			draft.composer,
			draft.date,
			draft.series,
			draft.series_part,
			draft.subseries,
			draft.subseries_part,
			String(draft.cover_art?.length ?? 0),
		].join('\0');
	});

	createEffect(() => {
		const directory = outputDirectory();
		const preset = namingPreset();
		const year = absIncludeYear();
		const committed = previewTemplate();
		const input = deps.input.view();
		metadataDraftKey();
		const metadata = untrack(() => deps.metadataView());
		const requestId = latestPreviewRequestId + 1;
		latestPreviewRequestId = requestId;

		const context = {
			outputDirectory: directory,
			sourcePath: sourcePathFromInput(input),
			outputNaming: outputNamingFromPlan({
				...emptyOutputPlan(),
				outputDirectory: directory,
				namingPreset: preset,
				previewTemplate: committed,
				absIncludeYear: year,
			}),
			metadataDraft: previewDraftFromMetadataView(metadata),
		};

		const layer = makeOutputPlanWorkflowServicesLayer({
			updateMetadataIntentWarnings: (draft) =>
				applyMetadataIntentWarnings(draft, deps.onMetadataValidation),
			beginOutputPreviewRequest: () => requestId,
			isLatestOutputPreviewRequest: (id) => id === latestPreviewRequestId,
			setOutputPreview: (text, title = text) => {
				setPreviewText(text);
				setPreviewTitle(title);
			},
			showOutputError,
			previewOutputPath: tauriClient.previewOutputPath,
			preflightProcessingPlan: tauriClient.preflightProcessingPlan,
			openCollisionDialog: () => Promise.resolve(null),
			console,
		});

		void runAppEffect(
			outputPathPreviewBody('final', context, requestId).pipe(Effect.provide(layer)),
		);
	});

	const owner: OutputPlanOwner = {
		view,
		estimatedSizeText,
		collision: collisionReview.view,
		applyDefaults(defaults) {
			clearTemplatePreviewTimer();
			const directory = defaults.outputDirectory ?? '';
			const preset = defaults.outputNaming.preset;
			const template = defaults.outputNaming.customTemplate ?? '';
			const year = defaults.outputNaming.includeYear;
			batch(() => {
				setOutputDirectory(directory);
				setNamingPreset(preset);
				setNamingTemplate(template);
				setPreviewTemplate(template);
				setAbsIncludeYearSignal(year);
			});
		},
		async browseDirectory() {
			try {
				const selectedPath = await tauriClient.openDirectory({
					title: 'Select Output Directory',
				});
				if (!selectedPath) {
					return;
				}
				clearTemplatePreviewTimer();
				setOutputDirectory(selectedPath);
				persistPlan();
			} catch (cause) {
				console.error('Error selecting directory:', cause);
				showOutputError('Failed to select directory');
			}
		},
		selectNamingPreset(value) {
			clearTemplatePreviewTimer();
			setNamingPreset(value === 'customTemplate' ? 'customTemplate' : 'absDefault');
			persistPlan();
		},
		setAbsIncludeYear(value) {
			clearTemplatePreviewTimer();
			setAbsIncludeYearSignal(value);
			persistPlan();
		},
		editNamingTemplate(value) {
			setNamingTemplate(value);
			scheduleCommittedTemplate();
		},
		openCollisionReview(plan) {
			return collisionReview.open(plan);
		},
		chooseCollisionPolicy(policy) {
			collisionReview.choose(policy);
		},
		cancelCollisionReview() {
			collisionReview.cancel();
		},
		readRequestConfig() {
			const directory = outputDirectory();
			if (!directory) {
				throw new Error('Output directory not selected');
			}
			return {
				outputDirectory: directory,
				outputNaming: outputNamingForSubmit(),
			};
		},
		readDefaults() {
			return {
				outputDirectory: outputDirectory() || undefined,
				outputNaming: outputNamingForSubmit(),
			};
		},
		reset() {
			collisionReview.reset();
			clearTemplatePreviewTimer();
			const next = emptyOutputPlan();
			latestPreviewRequestId = next.latestPreviewRequestId;
			batch(() => {
				setOutputDirectory(next.outputDirectory);
				setNamingPreset(next.namingPreset);
				setNamingTemplate(next.namingTemplate);
				setPreviewTemplate(next.previewTemplate);
				setAbsIncludeYearSignal(next.absIncludeYear);
				setPreviewText(next.previewText);
				setPreviewTitle(next.previewTitle);
			});
		},
	};

	bindOutputOwner(owner);
	onCleanup(() => {
		latestPreviewRequestId += 1;
		clearTemplatePreviewTimer();
		if (boundOutputOwner() === owner) {
			bindOutputOwner(undefined);
		}
	});

	return owner;
}
