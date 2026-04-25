export type HarnessScenarioId =
	| 'file-management'
	| 'metadata-edit'
	| 'status-processing'
	| 'output-preview'
	| 'collision-dialog';

export type HarnessScenarioVerifyCheck = {
	id: string;
	label: string;
};

export type HarnessScenarioReviewControl = {
	selector: string;
	label: string;
};

export type HarnessScenarioReviewAction =
	| {
			id: string;
			label: string;
			type: 'dialog-toggle';
			triggerSelector: string;
			dialogSelector: string;
			dismissSelector: string;
	  }
	| {
			id: string;
			label: string;
			type: 'select-option';
			selector: string;
			optionValue: string;
			assertVisibleSelector?: string;
			resetValue?: string;
	  }
	| {
			id: string;
			label: string;
			type: 'toggle-checkbox';
			selector: string;
	  }
	| {
			id: string;
			label: string;
			type: 'assert-text';
			selector: string;
			expectedText: string;
	  };

export type HarnessScenarioAdvisoryTarget = {
	selector: string;
	message: string;
};

export type HarnessScenario = {
	id: HarnessScenarioId;
	title: string;
	description: string;
	route: '/harness.html';
	screenshotName: string;
	matchers: readonly RegExp[];
	verifyChecks: readonly HarnessScenarioVerifyCheck[];
	review: {
		controls: readonly HarnessScenarioReviewControl[];
		actions: readonly HarnessScenarioReviewAction[];
		advisoryTargets?: readonly HarnessScenarioAdvisoryTarget[];
	};
};

const RUN_ALL_MATCHERS = [
	/^harness\.html$/,
	/^src\/App\.svelte$/,
	/^src\/HarnessApp\.svelte$/,
	/^src\/harness-main\.ts$/,
	/^src\/harness\//,
	/^src\/ui\/core\//,
	/^scripts\/harness\//,
	/^src\/styles\.css$/,
] as const;

const UI_SURFACE_MATCHERS = [
	/^src\/App\.svelte$/,
	/^src\/HarnessApp\.svelte$/,
	/^src\/styles\.css$/,
	/^src\/ui\//,
	/^src\/harness\//,
] as const;

const IGNORED_UI_PATH_MATCHERS = [/^src\/ui\/__tests__\//] as const;

const SCENARIOS: readonly HarnessScenario[] = [
	{
		id: 'file-management',
		title: 'File Management',
		description:
			'Verifies file import, selection context, reorder behavior, add-while-populated, and clear/reimport behavior.',
		route: '/harness.html',
		screenshotName: 'file-management.png',
		matchers: [
			/^src\/ui\/fileImport(?:\/|\.ts$)/,
			/^src\/ui\/fileList\/(?:actions|dom|events|index|inspectorState\.svelte|selection|state(?:\.svelte)?|viewState\.svelte)\.ts$/,
		],
		verifyChecks: [
			{
				id: 'selection-follows-reorder',
				label:
					'Reordering a selected file preserves the logical file selection and inspector context.',
			},
			{
				id: 'add-while-populated',
				label:
					'Adding files to a populated list appends the new selection without replacing the existing files.',
			},
			{
				id: 'clear-and-reimport',
				label:
					'Clearing the file list and re-importing restores the populated input lane with an empty inspector.',
			},
		],
		review: {
			controls: [
				{ selector: '.drop-zone-header', label: 'Add audio files header' },
				{ selector: '#sort-toggle-btn', label: 'Sort toggle button' },
				{ selector: '#clear-files-btn', label: 'Clear files button' },
				{ selector: '.file-list-content', label: 'File list content' },
			],
			actions: [
				{
					id: 'reorder-selection-context',
					label: 'Moving a selected file keeps the inspector bound to the same file.',
					type: 'assert-text',
					selector: '.inspector-context',
					expectedText: '02-dune-part-2.mp3',
				},
			],
			advisoryTargets: [
				{
					selector: '.file-management-container',
					message:
						'Review input-lane density so import affordances, file order controls, and inspector state read as one coherent surface.',
				},
			],
		},
	},
	{
		id: 'metadata-edit',
		title: 'Metadata Edit',
		description:
			'Verifies metadata form rendering, lookup apply behavior, and cover-art interactions.',
		route: '/harness.html',
		screenshotName: 'metadata-edit.png',
		matchers: [
			/^src\/ui\/metadataForm(?:\/|\.ts$)/,
			/^src\/ui\/metadataLookup(?:\/|\.ts$)/,
			/^src\/ui\/metadataDraft\.ts$/,
			/^src\/ui\/metadataSaveState\.ts$/,
			/^src\/ui\/metadataState\.ts$/,
			/^src\/ui\/coverArt(?:\/|\.ts$)/,
			/^src\/ui\/fileList\/metadataPanel\.ts$/,
			/^src\/ui\/tagPreview(?:\/|\.ts$)/,
			/^src\/types\/metadata(?:Intent)?\.ts$/,
		],
		verifyChecks: [
			{
				id: 'lookup-apply',
				label: 'Metadata lookup applies the selected result to the form.',
			},
			{
				id: 'cover-art-load',
				label: 'Cover-art URL loading updates the preview image and status message.',
			},
		],
		review: {
			controls: [
				{ selector: '[data-testid="metadata-lookup-btn"]', label: 'Metadata lookup button' },
				{ selector: '#meta-title', label: 'Book title field' },
				{ selector: '#metadata-save-btn', label: 'Metadata save button' },
			],
			actions: [
				{
					id: 'lookup-modal',
					label: 'Metadata lookup modal opens and closes cleanly.',
					type: 'dialog-toggle',
					triggerSelector: '[data-testid="metadata-lookup-btn"]',
					dialogSelector: '#metadata-lookup-modal',
					dismissSelector: '[data-testid="metadata-lookup-close"]',
				},
			],
			advisoryTargets: [
				{
					selector: '#metadata-content',
					message:
						'Review metadata form density and field rhythm; this lane should feel scannable rather than bulky.',
				},
			],
		},
	},
	{
		id: 'status-processing',
		title: 'Status Processing',
		description: 'Verifies status/progress rendering and per-job queue affordances.',
		route: '/harness.html',
		screenshotName: 'status-processing.png',
		matchers: [
			/^src\/ui\/statusPanel\//,
			/^src\/ui\/jobControls(?:\/|\.ts$)/,
			/^src\/types\/events\.ts$/,
		],
		verifyChecks: [
			{
				id: 'order-lock-visible',
				label: 'Processing locks file-order controls while work is in flight.',
			},
			{
				id: 'queue-completes',
				label: 'Queued processing reaches a completed 100.0% state.',
			},
		],
		review: {
			controls: [
				{ selector: '#process-button', label: 'Process audiobook button' },
				{ selector: '#percentage-processed', label: 'Progress percentage' },
				{ selector: '#status-text', label: 'Status text' },
				{ selector: '#job-list', label: 'Per-job queue list' },
			],
			actions: [
				{
					id: 'completed-progress',
					label: 'Progress percentage shows a completed 100.0% state.',
					type: 'assert-text',
					selector: '#percentage-processed',
					expectedText: '100.0%',
				},
				{
					id: 'completed-status',
					label: 'Status text shows a completed state.',
					type: 'assert-text',
					selector: '#status-text',
					expectedText: 'Completed',
				},
			],
			advisoryTargets: [
				{
					selector: '.status-panel',
					message:
						'Review status-panel density so progress, queue state, and actions read as one execution lane.',
				},
			],
		},
	},
	{
		id: 'output-preview',
		title: 'Output Preview',
		description:
			'Verifies output naming controls, encoder panel behavior, preview controls including duration propagation, and metadata-driven warnings.',
		route: '/harness.html',
		screenshotName: 'output-preview.png',
		matchers: [
			/^src\/ui\/outputPanel\//,
			/^src\/ui\/encoderPanel(?:\/|\.ts$)/,
			/^src\/ui\/previewAudio\//,
			/^src\/ui\/__tests__\/encoderPanel/,
			/^src\/ui\/metadataValidation\.ts$/,
			/^src\/ui\/metadataForm(?:\/|\.ts$)/,
			/^src\/ui\/tagPreview(?:\/|\.ts$)/,
		],
		verifyChecks: [
			{
				id: 'encoder-controls-reactive',
				label: 'Encoder controls react to availability and manual selection in the preview lane.',
			},
			{
				id: 'custom-template-row',
				label: 'Selecting the custom naming template reveals the template row.',
			},
			{
				id: 'preview-remains-anchored',
				label:
					'Output preview stays anchored to the chosen library path and resolves to a concrete .m4b artifact path.',
			},
			{
				id: 'preview-duration-propagates',
				label:
					'Selecting a non-default preview duration from the dropdown reaches the processing call.',
			},
		],
		review: {
			controls: [
				{ selector: '#output-dir-browse', label: 'Output directory browse button' },
				{ selector: '#output-naming-preset', label: 'Output naming preset' },
				{ selector: '#output-abs-include-year', label: 'Include year toggle' },
				{ selector: '#preview-dropdown-toggle', label: 'Preview duration dropdown' },
				{ selector: '#output-preview-text', label: 'Output preview text' },
			],
			actions: [
				{
					id: 'custom-template-toggle',
					label: 'Output naming preset reveals the custom template row.',
					type: 'select-option',
					selector: '#output-naming-preset',
					optionValue: 'customTemplate',
					assertVisibleSelector: '#output-template-row',
					resetValue: 'absDefault',
				},
				{
					id: 'abs-include-year-toggle',
					label: 'ABS include-year toggle changes and restores cleanly.',
					type: 'toggle-checkbox',
					selector: '#output-abs-include-year',
				},
			],
			advisoryTargets: [
				{
					selector: '.output-options-panel',
					message:
						'Review output controls for wasted space and hierarchy drift; secondary options should stay visually subordinate.',
				},
				{
					selector: '#output-preview-text',
					message:
						'Review preview copy density and truncation behavior; the path preview should stay legible without dominating the panel.',
				},
			],
		},
	},
	{
		id: 'collision-dialog',
		title: 'Collision Dialog',
		description:
			'Verifies collision preflight opens a batch dialog, shows preview artifact naming, and applies the chosen policy before processing starts.',
		route: '/harness.html',
		screenshotName: 'collision-dialog.png',
		matchers: [
			/^src\/ui\/collisionDialog\//,
			/^src\/ui\/previewAudio\//,
			/^src\/ui\/statusPanel\//,
			/^src\/lib\/tauri\//,
			/^src\/types\/audio\.ts$/,
			/^src\/harness\//,
		],
		verifyChecks: [
			{
				id: 'collision-dialog-opens',
				label: 'Starting processing with planned collisions opens the batch collision dialog.',
			},
			{
				id: 'preview-artifact-path-visible',
				label: 'The collision dialog shows preview artifact naming with the .preview.m4b suffix.',
			},
			{
				id: 'skip-policy-propagates',
				label:
					'Choosing Skip Existing propagates into processing and produces a skipped batch summary.',
			},
		],
		review: {
			controls: [
				{ selector: '#collision-dialog-modal', label: 'Collision dialog modal' },
				{ selector: '#collision-dialog-results', label: 'Collision dialog result list' },
				{ selector: '#collision-dialog-skip', label: 'Skip existing button' },
			],
			actions: [
				{
					id: 'collision-dialog-toggle',
					label: 'Collision dialog opens and closes cleanly.',
					type: 'dialog-toggle',
					triggerSelector: '#preview-button',
					dialogSelector: '#collision-dialog-modal',
					dismissSelector: '#collision-dialog-cancel',
				},
			],
		},
	},
] as const;

function normalizePath(path: string): string {
	return path.replace(/\\/g, '/').replace(/^\.\//, '');
}

function matchesAny(path: string, matchers: readonly RegExp[]): boolean {
	return matchers.some((matcher) => matcher.test(path));
}

export function listHarnessScenarios(): readonly HarnessScenario[] {
	return SCENARIOS;
}

export function getHarnessScenario(id: HarnessScenarioId): HarnessScenario {
	const scenario = SCENARIOS.find((candidate) => candidate.id === id);
	if (!scenario) {
		throw new Error(`Unknown harness scenario: ${id}`);
	}
	return scenario;
}

export function resolveHarnessScenariosForPaths(paths: string[]): HarnessScenario[] {
	const normalizedPaths = paths.map(normalizePath);
	if (normalizedPaths.some((path) => matchesAny(path, RUN_ALL_MATCHERS))) {
		return [...SCENARIOS];
	}

	const matched = SCENARIOS.filter((scenario) =>
		normalizedPaths.some((path) => matchesAny(path, scenario.matchers)),
	);

	return [...matched];
}

export function findUnmappedHarnessUiPaths(paths: string[]): string[] {
	const normalizedPaths = paths.map(normalizePath);
	return normalizedPaths.filter((path) => {
		if (!matchesAny(path, UI_SURFACE_MATCHERS)) {
			return false;
		}
		if (matchesAny(path, IGNORED_UI_PATH_MATCHERS)) {
			return false;
		}
		if (matchesAny(path, RUN_ALL_MATCHERS)) {
			return false;
		}
		return !SCENARIOS.some((scenario) => matchesAny(path, scenario.matchers));
	});
}
