// TEMPORARY browser harness for the idle-redraw / remote-view PR. Removed before merge.
import '../styles.css';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { render } from '@solidjs/web';
import { flush } from 'solid-js';
import { AppRuntimeProvider, createAppRuntime } from '../app/runtime';
import { createFakeEngine } from '../test/fixtures/fakeEngine';
import { App } from '../ui/App';
import { progressTick, seedDownload, tickableEngine } from './fixture';

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'idle';
const titleCount = Number(params.get('titles') ?? '1000');
if (params.get('noanim') === '1') {
	const style = document.createElement('style');
	style.textContent =
		'*, *::before, *::after { animation: none !important; transition: none !important; }';
	document.head.append(style);
}

mockWindows('main');
mockIPC(
	(command) => {
		if (command === 'list_work_operations') return { revision: 0, order: [], operations: [] };
		return null;
	},
	{ shouldMockEvents: true },
);

const engine = createFakeEngine();
const tickable = tickableEngine(engine);
const runtime = createAppRuntime({ engine: tickable.capability });
const seeded = scenario === 'idle' ? null : seedDownload(engine, titleCount);

if (scenario === 'idle-complete') {
	engine.change((state) => {
		state.output.previewRun = {
			openReady: false,
			artworkReady: false,
			operation: {
				operationId: 'perf-preview',
				sequence: 0,
				revision: 2,
				kind: 'processingBatch',
				status: 'completed',
				title: 'Synthetic preview',
				createdAtMs: 0,
				startedAtMs: 1,
				finishedAtMs: 2,
				cancellable: false,
				cancelRequested: false,
				lanes: ['encodeCpu'],
				sourceInputIds: [],
				progress: {
					stage: 'complete',
					percentage: 100,
					message: 'Done.',
					currentItemIndex: 1,
					totalItems: 1,
					bytesDownloaded: null,
					bytesTotal: null,
					etaSeconds: null,
				},
				terminalSummary: null,
				errors: [],
				logTail: [],
				children: [],
			},
		} as never;
	});
}

const target = document.getElementById('app') as HTMLElement;
render(
	() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	),
	target,
);

declare global {
	interface Window {
		__abbPerf: {
			ready: boolean;
			received: () => number;
			tick: (percentage: number) => number;
			closeRemote: () => void;
			setFilter: (text: string) => void;
			rows: () => number;
		};
	}
}

window.__abbPerf = {
	ready: false,
	received: tickable.received,
	tick(percentage) {
		if (!seeded) return 0;
		const start = performance.now();
		tickable.emit(progressTick(seeded, percentage));
		flush();
		return performance.now() - start;
	},
	closeRemote: () => runtime.remoteSource.close(),
	setFilter: (text) => runtime.remoteSource.editSearch({ titleFilter: text }),
	rows: () => document.querySelectorAll('.remote-title-row').length,
};

void (async () => {
	await runtime.initialize();
	if (seeded) await runtime.remoteSource.open({ lane: 'audible' });
	if (scenario === 'download-closed') runtime.remoteSource.close();
	const autotick = Number(params.get('autotick') ?? '0');
	if (autotick > 0 && seeded) {
		let percentage = 0;
		setInterval(() => {
			percentage = (percentage + 0.1) % 100;
			window.__abbPerf.tick(percentage);
		}, 1000 / autotick);
	}
	window.__abbPerf.ready = true;
	document.title = `abb-perf ready ${scenario}`;
})();
