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
	TitleAudioPlan,
	AudioFile,
	OutputNamingConfig,
	OutputRequestConfig,
	ProcessingPreflightPlan,
} from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import type { EncodingOwner } from '../encoding';
import type { InputOwner } from '../inputSession';
import type { MetadataDraftValidation, MetadataView } from '../metadataSession';
import { createCollisionReview, type CollisionView } from './collision';
import { estimateEncodedSizeBytes } from './estimate';
import { formatFileSize } from '../../types/audio';
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

type PreviewPlanBag = {
	outputDirectory: string;
	namingPreset: OutputPlanState['namingPreset'];
	previewTemplate: string;
	absIncludeYear: boolean;
};

export type OutputPlanOwner = {
	readonly view: Accessor<OutputView>;
	estimateTitleSizeText(file: AudioFile, resolvedPlan?: TitleAudioPlan): string | null;
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
	readonly persistDefaults: (defaults: OutputDefaults) => void;
	readonly input: InputOwner;
	readonly metadataView: Accessor<MetadataView>;
	readonly encoding: Pick<EncodingOwner, 'audioRequest' | 'estimateTitleKbps'>;
	readonly onMetadataValidation?: (validation: MetadataDraftValidation) => void;
};

export function createOutputOwner(deps: OutputOwnerDeps): OutputPlanOwner {
	const empty = emptyOutputPlan();
	let previewPlan: PreviewPlanBag = {
		outputDirectory: empty.outputDirectory,
		namingPreset: empty.namingPreset,
		previewTemplate: empty.previewTemplate,
		absIncludeYear: empty.absIncludeYear,
	};
	let namingTemplate = empty.namingTemplate;
	const [previewRev, bumpPreview] = createSignal(0, { ownedWrite: true });
	const [formRev, bumpForm] = createSignal(0, { ownedWrite: true });
	const [previewText, setPreviewText] = createSignal(empty.previewText);
	const [previewTitle, setPreviewTitle] = createSignal(empty.previewTitle);
	let templatePreviewTimer: ReturnType<typeof setTimeout> | null = null;
	const collisionReview = createCollisionReview();

	function commitPreviewPlan(next: PreviewPlanBag): void {
		previewPlan = next;
		bumpPreview((n) => n + 1);
	}

	function commitLiveTemplate(next: string): void {
		namingTemplate = next;
		bumpForm((n) => n + 1);
	}

	function namingFields(): OutputPlanState {
		return {
			outputDirectory: previewPlan.outputDirectory,
			namingPreset: previewPlan.namingPreset,
			namingTemplate,
			previewTemplate: previewPlan.previewTemplate,
			absIncludeYear: previewPlan.absIncludeYear,
			previewText: EMPTY_PREVIEW_TEXT,
			previewTitle: EMPTY_PREVIEW_TITLE,
		};
	}

	function estimateTitleSizeText(file: AudioFile, resolvedPlan?: TitleAudioPlan): string | null {
		if (!file.isValid) return null;
		const request = deps.encoding.audioRequest(file);
		const handling = request.intent === 'auto' ? resolvedPlan?.handling : request.intent;
		// Default can copy or encode; wait for the backend's title preview.
		if (!handling) return null;
		const sources = deps.input.sourcesFor(file);
		if (sources.length === 0) return null;
		if (handling === 'preserve') {
			if (sources.some((source) => source.size === undefined)) return null;
			const bytes = sources.reduce((total, source) => total + (source.size ?? 0), 0);
			return `Est. ~ ${formatFileSize(bytes)}`;
		}
		const kbps = deps.encoding.estimateTitleKbps(file, resolvedPlan);
		if (sources.some((source) => source.duration === undefined)) return null;
		if (kbps === null) return 'Size varies with audio';
		const duration = sources.reduce((total, source) => total + (source.duration ?? 0), 0);
		const rough = resolvedPlan?.settings?.bitrateMode.mode === 'vbr';
		return `${rough ? 'Rough est.' : 'Est.'} ~ ${formatFileSize(estimateEncodedSizeBytes(duration, kbps))}${rough ? ' · varies with audio' : ''}`;
	}

	const view: Accessor<OutputView> = () => {
		formRev();
		previewRev();
		const directory = previewPlan.outputDirectory;
		const preset = previewPlan.namingPreset;
		const year = previewPlan.absIncludeYear;
		return {
			outputDirectory: directory,
			namingPreset: preset,
			namingTemplate,
			absIncludeYear: year,
			previewText: previewText(),
			previewTitle: previewTitle(),
			absHintText: namingHintText(preset, year),
			absHintHidden: preset !== 'absDefault',
			templateRowHidden: preset !== 'customTemplate',
			displayDirectory: directory || EMPTY_PREVIEW_TEXT,
		};
	};

	function persistPlan(overrides: Partial<PreviewPlanBag> = {}): void {
		const next = { ...previewPlan, ...overrides };
		deps.persistDefaults({
			outputDirectory: next.outputDirectory || undefined,
			outputNaming: outputNamingFromPlan({
				...namingFields(),
				...next,
			}),
		});
	}

	function outputNamingForSubmit(): OutputNamingConfig {
		return outputNamingFromPlan({
			...namingFields(),
			previewTemplate: namingTemplate,
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
			commitPreviewPlan({ ...previewPlan, previewTemplate: namingTemplate });
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

	const previewFormat = createMemo(() => {
		const input = deps.input.view();
		return deps.encoding.audioRequest(
			input.files.find((file) => file.path === sourcePathFromInput(input)),
		).format;
	});
	const previewContext = createMemo(() => {
		previewRev();
		const directory = previewPlan.outputDirectory;
		const preset = previewPlan.namingPreset;
		const year = previewPlan.absIncludeYear;
		const committed = previewPlan.previewTemplate;
		const input = deps.input.view();
		metadataDraftKey();
		const metadata = untrack(() => deps.metadataView());
		const sourcePath = sourcePathFromInput(input);
		return {
			outputDirectory: directory,
			sourcePath,
			format: previewFormat(),
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
		estimateTitleSizeText,
		collision: collisionReview.view,
		applyDefaults(defaults) {
			clearTemplatePreviewTimer();
			const template = defaults.outputNaming.customTemplate ?? '';
			commitLiveTemplate(template);
			commitPreviewPlan({
				outputDirectory: defaults.outputDirectory ?? '',
				namingPreset: defaults.outputNaming.preset,
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
				commitPreviewPlan({ ...previewPlan, outputDirectory: selectedPath });
				persistPlan({ outputDirectory: selectedPath });
			} catch (cause) {
				console.error('Error selecting directory:', cause);
				showOutputError('Failed to select directory');
			}
		},
		selectNamingPreset(value) {
			clearTemplatePreviewTimer();
			const preset = value === 'customTemplate' ? 'customTemplate' : 'absDefault';
			commitPreviewPlan({ ...previewPlan, namingPreset: preset });
			persistPlan({ namingPreset: preset });
		},
		setAbsIncludeYear(value) {
			clearTemplatePreviewTimer();
			commitPreviewPlan({ ...previewPlan, absIncludeYear: value });
			persistPlan({ absIncludeYear: value });
		},
		editNamingTemplate(value) {
			commitLiveTemplate(value);
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
			const directory = previewPlan.outputDirectory;
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
				outputDirectory: previewPlan.outputDirectory || undefined,
				outputNaming: outputNamingForSubmit(),
			};
		},
		reset() {
			collisionReview.reset();
			clearTemplatePreviewTimer();
			const next = emptyOutputPlan();
			commitLiveTemplate(next.namingTemplate);
			commitPreviewPlan({
				outputDirectory: next.outputDirectory,
				namingPreset: next.namingPreset,
				previewTemplate: next.previewTemplate,
				absIncludeYear: next.absIncludeYear,
			});
			setPreviewText(next.previewText);
			setPreviewTitle(next.previewTitle);
		},
	};

	onCleanup(() => {
		clearTemplatePreviewTimer();
	});

	return owner;
}
