export type HarnessScenarioId = 'metadata-edit' | 'status-processing' | 'output-preview';

export type HarnessScenario = {
	id: HarnessScenarioId;
	title: string;
	description: string;
	route: '/harness.html';
	screenshotName: string;
	matchers: readonly RegExp[];
};

const RUN_ALL_MATCHERS = [
	/^harness\.html$/,
	/^src\/App\.svelte$/,
	/^src\/HarnessApp\.svelte$/,
	/^src\/harness-main\.ts$/,
	/^src\/harness\//,
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

const SCENARIOS: readonly HarnessScenario[] = [
	{
		id: 'metadata-edit',
		title: 'Metadata Edit',
		description: 'Verifies metadata form rendering, dirty-state actions, and lookup modal access.',
		route: '/harness.html',
		screenshotName: 'metadata-edit.png',
		matchers: [
			/^src\/ui\/metadataForm(?:\/|\.ts$)/,
			/^src\/ui\/metadataLookup(?:\/|\.ts$)/,
			/^src\/ui\/metadataState\.ts$/,
			/^src\/ui\/coverArt(?:\/|\.ts$)/,
			/^src\/ui\/tagPreview(?:\/|\.ts$)/,
			/^src\/types\/metadata(?:Intent)?\.ts$/,
		],
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
	},
	{
		id: 'output-preview',
		title: 'Output Preview',
		description: 'Verifies output naming controls, preview fallback, and metadata-driven warnings.',
		route: '/harness.html',
		screenshotName: 'output-preview.png',
		matchers: [
			/^src\/ui\/outputPanel\//,
			/^src\/ui\/metadataValidation\.ts$/,
			/^src\/ui\/metadataForm(?:\/|\.ts$)/,
			/^src\/ui\/tagPreview(?:\/|\.ts$)/,
		],
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
		if (matchesAny(path, RUN_ALL_MATCHERS)) {
			return false;
		}
		return !SCENARIOS.some((scenario) => matchesAny(path, scenario.matchers));
	});
}
