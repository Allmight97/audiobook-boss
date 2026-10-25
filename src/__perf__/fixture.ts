// TEMPORARY measurement fixture for the idle-redraw / remote-view PR. Removed before merge.
import { normalizeSessionUpdate } from '../lib/tauri/normalizers';
import type { EngineCapability } from '../lib/tauri/capabilities/engine';
import type { SessionUpdate as GeneratedSessionUpdate } from '../lib/generated/tauri';
import type { FakeEngine } from '../test/fixtures/fakeEngine';
import type { AcquisitionJob, RemoteTitle } from '../types/remoteSource';
import type { RemoteUiSnapshot, SessionUpdate } from '../types/session';

export function remoteTitles(count: number): RemoteTitle[] {
	return Array.from({ length: count }, (_, index) => ({
		providerId: 'audible',
		titleId: `B${String(index).padStart(9, '0')}`,
		title: `Synthetic Book ${index} ${['Dune', 'Emma', 'Ulysses', 'Beloved', 'Ivanhoe'][index % 5]}`,
		authors: [`Author ${index % 97}`],
		narrators: [`Narrator ${index % 31}`],
		durationSeconds: 3600 + index,
		supplementalPdfAvailable: index % 5 === 0,
		acquired: false,
		availability:
			index % 11 === 0
				? { status: 'unavailable', acquirable: false, label: 'Not available in this region' }
				: { status: 'available', acquirable: true, label: 'Available' },
		unsupportedReasons: [],
	})) as RemoteTitle[];
}

function job(percentage: number): AcquisitionJob {
	return {
		settled: false,
		terminal: false,
		jobId: 'perf-job',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage,
			message: 'Downloading audiobook.',
			terminal: false,
			currentTitleId: 'B000000003',
			currentItemIndex: 1,
			totalItems: 1,
		},
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
	} as AcquisitionJob;
}

/** Wraps a fake engine so the harness can push production-shaped, remote-only updates. */
export function tickableEngine(engine: FakeEngine): {
	readonly capability: EngineCapability;
	readonly emit: (update: SessionUpdate) => void;
	readonly received: () => number;
} {
	const handlers = new Set<(update: SessionUpdate) => void>();
	let received = 0;
	const capability: EngineCapability = {
		...engine,
		listenSessionUpdates: async (handler) => {
			const counted = (update: SessionUpdate) => {
				received += 1;
				handler(update);
			};
			handlers.add(counted);
			const unlisten = await engine.listenSessionUpdates(counted);
			return () => {
				handlers.delete(counted);
				unlisten();
			};
		},
	};
	return {
		capability,
		emit: (update) => {
			for (const handler of handlers) handler(update);
		},
		received: () => received,
	};
}

export type Seeded = {
	readonly titles: RemoteTitle[];
	remote: RemoteUiSnapshot;
	revision: number;
};

/** Seeds a connected Audible library with an active download, as Rust snapshots it. */
export function seedDownload(engine: FakeEngine, count: number, selected = 50): Seeded {
	const titles = remoteTitles(count);
	let remote: RemoteUiSnapshot | undefined;
	engine.change((state) => {
		state.remote.lane = 'audible';
		state.remote.providers = [
			{ providerId: 'audible', label: 'Audible' },
			{ providerId: 'indexer', label: 'Indexer' },
		];
		state.remote.account = { providerId: 'audible', status: 'connected' };
		state.remote.libraryStatus = { kind: 'succeeded' };
		state.remote.selectedTitleIds = titles.slice(0, selected).map((title) => title.titleId);
		state.remote.includePdfByTitleId = Object.fromEntries(
			titles.map((title) => [title.titleId, title.supplementalPdfAvailable]),
		);
		state.remoteAcquisition.job = job(0);
		state.remoteAcquisition.acquiring = true;
		state.remoteLibrary.titles = titles;
		remote = structuredClone(state.remote);
	});
	return { titles, remote: remote as RemoteUiSnapshot, revision: 1_000_000 };
}

/** One throttled download tick as the engine now sends it: only the acquisition part. */
export function progressTick(seeded: Seeded, percentage: number): SessionUpdate {
	seeded.revision += 1;
	const wire = JSON.parse(
		JSON.stringify({
			revision: seeded.revision,
			remoteAcquisition: { revision: seeded.revision, acquiring: false, job: job(percentage) },
		}),
	) as GeneratedSessionUpdate;
	return normalizeSessionUpdate(wire);
}
