import {
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
	EMPTY_PREVIEW_TITLE,
	type OutputPlanState,
	type OutputView,
} from './types';
import {
	computeOutputPathPreview,
	showOutputError,
	updateMetadataIntentWarnings as applyMetadataIntentWarnings,
	type OutputPathPreviewResult,
} from './workflow';

const TEMPLATE_PREVIEW_DEBOUNCE_MS = 150;

const EMPTY_PREVIEW_RESULT: OutputPathPreviewResult = {
	ok: true,
	text: EMPTY_PREVIEW_TEXT,
	title: EMPTY_PREVIEW_TITLE,
};

type OutputPlanBag = {
	outputDirectory: string;
	namingPreset: OutputPlanState['namingPreset'];
	namingTemplate: string;
	previewTemplate: string;
	absIncludeYear: boolean;
};

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
	let plan: OutputPlanBag = {
		outputDirectory: empty.outputDirectory,
		namingPreset: empty.namingPreset,
		namingTemplate: empty.namingTemplate,
		previewTemplate: empty.previewTemplate,
		absIncludeYear: empty.absIncludeYear,
	};
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const [previewText, setPreviewText] = createSignal(empty.previewText);
	const [previewTitle, setPreviewTitle] = createSignal(empty.previewTitle);
	let templatePreviewTimer: ReturnType<typeof setTimeout> | null = null;
	const collisionReview = createCollisionReview();

	function commitPlan(next: OutputPlanBag): void {
		plan = next;
		bump((n) => n + 1);
	}

	const estimatedSizeText = createMemo(() => {
		const input = deps.input.view();
		const request = deps.encodingRequest();
		return formatEstimatedSizeText(input.hasFiles, input.totalDurationSeconds, {
			bitrateKbps: deps.encodingEstimateKbps(),
			channels: request.encoderSettings.channels,
		});
	});

	const view: Accessor<OutputView> = () => {
		rev();
		const directory = plan.outputDirectory;
		const preset = plan.namingPreset;
		const year = plan.absIncludeYear;
		return {
			outputDirectory: directory,
			namingPreset: preset,
			namingTemplate: plan.namingTemplate,
			absIncludeYear: year,
			previewText: previewText(),
			previewTitle: previewTitle(),
			absHintText: namingHintText(preset, year),
			absHintHidden: preset !== 'absDefault',
			templateRowHidden: preset !== 'customTemplate',
			displayDirectory: directory || EMPTY_PREVIEW_TEXT,
			estimatedSizeText: estimatedSizeText(),
		};
	};

	function persistPlan(overrides: Partial<OutputPlanBag> = {}): void {
		const next = { ...plan, ...overrides };
		void persistOutputDefaults({
			outputDirectory: next.outputDirectory || undefined,
			outputNaming: outputNamingFromPlan({
				...next,
				previewText: previewText(),
				previewTitle: previewTitle(),
			}),
		});
	}

	function outputNamingForSubmit(): OutputNamingConfig {
		return outputNamingFromPlan({
			...plan,
			previewTemplate: plan.namingTemplate,
			previewText: previewText(),
			previewTitle: previewTitle(),
		});
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
			commitPlan({ ...plan, previewTemplate: plan.namingTemplate });
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

	const previewContext = createMemo(() => {
		rev();
		const directory = plan.outputDirectory;
		const preset = plan.namingPreset;
		const year = plan.absIncludeYear;
		const committed = plan.previewTemplate;
		const input = deps.input.view();
		metadataDraftKey();
		const metadata = untrack(() => deps.metadataView());
		return {
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
	});

	const previewQuery = createMemo(
		async () => {
			const context = previewContext();
			return computeOutputPathPreview('final', context, tauriClient.previewOutputPath);
		},
		{ loadingValue: EMPTY_PREVIEW_RESULT },
	);

	createEffect(
		() => previewQuery(),
		(preview) => {
			setPreviewText(preview.text);
			setPreviewTitle(preview.title);
			if (!preview.ok) {
				showOutputError(`Rust preview failed: ${String(preview.cause)}`);
			}
		},
	);

	createEffect(
		() => previewContext().metadataDraft,
		(draft) => {
			void applyMetadataIntentWarnings(draft, deps.onMetadataValidation).catch((error) => {
				console.error('Metadata preview validation failed:', error);
				showOutputError('Failed to validate metadata preview.');
			});
		},
	);

	const owner: OutputPlanOwner = {
		view,
		estimatedSizeText,
		collision: collisionReview.view,
		applyDefaults(defaults) {
			clearTemplatePreviewTimer();
			const template = defaults.outputNaming.customTemplate ?? '';
			commitPlan({
				outputDirectory: defaults.outputDirectory ?? '',
				namingPreset: defaults.outputNaming.preset,
				namingTemplate: template,
				previewTemplate: template,
				absIncludeYear: defaults.outputNaming.includeYear,
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
				commitPlan({ ...plan, outputDirectory: selectedPath });
				persistPlan({ outputDirectory: selectedPath });
			} catch (cause) {
				console.error('Error selecting directory:', cause);
				showOutputError('Failed to select directory');
			}
		},
		selectNamingPreset(value) {
			clearTemplatePreviewTimer();
			const preset = value === 'customTemplate' ? 'customTemplate' : 'absDefault';
			commitPlan({ ...plan, namingPreset: preset });
			persistPlan({ namingPreset: preset });
		},
		setAbsIncludeYear(value) {
			clearTemplatePreviewTimer();
			commitPlan({ ...plan, absIncludeYear: value });
			persistPlan({ absIncludeYear: value });
		},
		editNamingTemplate(value) {
			commitPlan({ ...plan, namingTemplate: value });
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
			const directory = plan.outputDirectory;
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
				outputDirectory: plan.outputDirectory || undefined,
				outputNaming: outputNamingForSubmit(),
			};
		},
		reset() {
			collisionReview.reset();
			clearTemplatePreviewTimer();
			const next = emptyOutputPlan();
			commitPlan({
				outputDirectory: next.outputDirectory,
				namingPreset: next.namingPreset,
				namingTemplate: next.namingTemplate,
				previewTemplate: next.previewTemplate,
				absIncludeYear: next.absIncludeYear,
			});
			setPreviewText(next.previewText);
			setPreviewTitle(next.previewTitle);
		},
	};

	bindOutputOwner(owner);
	onCleanup(() => {
		clearTemplatePreviewTimer();
		if (boundOutputOwner() === owner) {
			bindOutputOwner(undefined);
		}
	});

	return owner;
}
