export interface HarnessToggleFixture {
	enabled: boolean;
}

export interface HarnessLabelsFixture {
	inputPanelTitle: string;
	metadataLookupTitle: string;
}

export interface HarnessIslandsFixture {
	jobControls: HarnessToggleFixture;
	fileImport: HarnessToggleFixture;
	coverArt: HarnessToggleFixture;
	metadataForm: HarnessToggleFixture;
	encoderPanel: HarnessToggleFixture;
	outputPanel: HarnessToggleFixture;
	tagPreview: HarnessToggleFixture;
	statusPanel: HarnessToggleFixture;
	metadataLookup: HarnessToggleFixture;
}

export interface HarnessFixture {
	labels: HarnessLabelsFixture;
	islands: HarnessIslandsFixture;
}

export interface PartialHarnessFixture {
	labels?: Partial<HarnessLabelsFixture>;
	islands?: {
		[K in keyof HarnessIslandsFixture]?: Partial<HarnessToggleFixture>;
	};
}

const DEFAULT_HARNESS_FIXTURE: HarnessFixture = {
	labels: {
		inputPanelTitle: 'Harness: Input',
		metadataLookupTitle: 'Harness: Metadata Lookup Modal',
	},
	islands: {
		jobControls: { enabled: true },
		fileImport: { enabled: true },
		coverArt: { enabled: true },
		metadataForm: { enabled: true },
		encoderPanel: { enabled: true },
		outputPanel: { enabled: true },
		tagPreview: { enabled: true },
		statusPanel: { enabled: true },
		metadataLookup: { enabled: true },
	},
};

function resolveToggle(
	defaultToggle: HarnessToggleFixture,
	override?: Partial<HarnessToggleFixture>,
): HarnessToggleFixture {
	return {
		enabled: override?.enabled ?? defaultToggle.enabled,
	};
}

export function createHarnessFixture(overrides: PartialHarnessFixture = {}): HarnessFixture {
	const labels = overrides.labels ?? {};
	const islands = overrides.islands ?? {};
	const defaultIslands = DEFAULT_HARNESS_FIXTURE.islands;

	return {
		labels: {
			inputPanelTitle: labels.inputPanelTitle ?? DEFAULT_HARNESS_FIXTURE.labels.inputPanelTitle,
			metadataLookupTitle:
				labels.metadataLookupTitle ?? DEFAULT_HARNESS_FIXTURE.labels.metadataLookupTitle,
		},
		islands: {
			jobControls: resolveToggle(defaultIslands.jobControls, islands.jobControls),
			fileImport: resolveToggle(defaultIslands.fileImport, islands.fileImport),
			coverArt: resolveToggle(defaultIslands.coverArt, islands.coverArt),
			metadataForm: resolveToggle(defaultIslands.metadataForm, islands.metadataForm),
			encoderPanel: resolveToggle(defaultIslands.encoderPanel, islands.encoderPanel),
			outputPanel: resolveToggle(defaultIslands.outputPanel, islands.outputPanel),
			tagPreview: resolveToggle(defaultIslands.tagPreview, islands.tagPreview),
			statusPanel: resolveToggle(defaultIslands.statusPanel, islands.statusPanel),
			metadataLookup: resolveToggle(defaultIslands.metadataLookup, islands.metadataLookup),
		},
	};
}

export function getDefaultHarnessFixture(): HarnessFixture {
	return createHarnessFixture();
}
