import { afterEach, expect, it, vi } from 'vitest';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';
import type { AcquisitionJob } from '../../types/remoteSource';
import type { RemoteSourceWorkflowServices } from './workflow';

const runtimes: AppRuntime[] = [];
afterEach(() => {
	for (const runtime of runtimes.splice(0)) runtime.dispose();
});
const job: AcquisitionJob = {
	jobId: 'download-1',
	providerId: 'audible',
	status: 'acquiring',
	terminal: false,
	settled: false,
	progress: {
		stage: 'download',
		percentage: 40,
		message: 'Downloading audiobook.',
		terminal: false,
	},
	materializedFiles: [],
	supplementalAssets: [],
	diagnostics: [],
};
function services(): RemoteSourceWorkflowServices {
	return {
		listProviders: vi.fn(async () => []),
		getAccountState: vi.fn(async (providerId) => ({ providerId, status: 'connected' as const })),
		startAuth: vi.fn(),
		openAuthorizationUrl: vi.fn(),
		completeAuth: vi.fn(),
		logout: vi.fn(),
		loadLibrary: vi.fn(async () => ({
			providerId: 'audible' as const,
			titles: [],
			diagnostics: [],
		})),
	};
}
async function open(engine: FakeEngine = createFakeEngine(), capability = services()) {
	const runtime = createAppRuntime({ engine, remoteSource: { services: capability } });
	runtimes.push(runtime);
	await runtime.initialize();
	await runtime.remoteSource.open();
	return { runtime, engine, services: capability, owner: runtime.remoteSource };
}
it('renders acquisition progress from the attached session and Close leaves it running', async () => {
	const engine = createFakeEngine();
	engine.change((state) => {
		state.remote.acquisition = job;
	});
	const { owner, runtime } = await open(engine);
	expect(owner.view().activeJob?.progress.percentage).toBe(40);
	expect(owner.view().isBusy).toBe(true);
	owner.close();
	expect(engine.sessionIntents).not.toContainEqual({
		kind: 'remote',
		intent: { kind: 'cancelAcquisition', jobId: job.jobId },
	});
	runtime.dispose();
	const replacement = await open(engine);
	expect(replacement.owner.view().activeJob?.jobId).toBe(job.jobId);
	await replacement.owner.runAction({ type: 'cancelActiveAcquisition' });
	expect(engine.sessionIntents).toContainEqual({
		kind: 'remote',
		intent: { kind: 'cancelAcquisition', jobId: job.jobId },
	});
});
it('words the engine handoff and keeps Indexer status independent of an Audible result', async () => {
	const { owner, engine } = await open();
	engine.change((state) => {
		state.remote.acquisition = {
			...job,
			settled: true,
			terminal: true,
			status: 'importedToFileList',
			handoff: { kind: 'imported', count: 2 },
		};
	});
	expect(owner.view().statusMessage).toBe('2 acquired titles imported.');
	await owner.selectLane('indexer');
	engine.change((state) => {
		state.remote.indexer.message = 'Sending one release';
		state.remote.indexer.grabbing = true;
	});
	expect(owner.view().statusMessage).toBe('Sending one release');
	expect(owner.view().isBusy).toBe(true);
});
it('passes title, PDF, search, and batch choices as intents while showing engine facts', async () => {
	const { owner, engine } = await open();
	owner.toggleTitle('book-1');
	owner.toggleSupplementalPdf('book-1');
	await vi.waitFor(() =>
		expect(engine.sessionIntents).toContainEqual({
			kind: 'remote',
			intent: { kind: 'togglePdf', titleId: 'book-1' },
		}),
	);
	await owner.selectLane('indexer');
	owner.editSearch({ indexerAuthorQuery: 'Writer', indexerTitleQuery: 'Book' });
	await owner.runAction({ type: 'searchReleases' });
	await owner.runAction({ type: 'grabSelectedReleases' });
	expect(engine.sessionIntents).toContainEqual({
		kind: 'remote',
		intent: { kind: 'searchReleases', author: 'Writer', title: 'Book' },
	});
	expect(engine.sessionIntents).toContainEqual({
		kind: 'remote',
		intent: { kind: 'grabSelected' },
	});
	engine.change((state) => {
		state.remote.selectedTitleIds = ['book-1'];
		state.remote.includePdfByTitleId = { 'book-1': false };
	});
	expect([...owner.view().selectedTitleIds]).toEqual(['book-1']);
	expect(owner.view().includePdfByTitleId['book-1']).toBe(false);
});
it('hydrates the selected provider and scans only connected Audible libraries', async () => {
	const { owner, services: capability } = await open();
	expect(capability.loadLibrary).toHaveBeenCalledExactlyOnceWith('audible');
	await owner.selectLane('indexer');
	expect(capability.getAccountState).toHaveBeenLastCalledWith('indexer');
	expect(capability.loadLibrary).toHaveBeenCalledTimes(1);
});

it('shows a new engine refusal instead of hiding it under an older acquisition result', async () => {
	const { owner, engine } = await open();
	engine.change((state) => {
		state.remote.acquisition = {
			...job,
			settled: true,
			terminal: true,
			status: 'failed',
			progress: { ...job.progress, message: 'Old download failure', terminal: true },
		};
	});
	engine.respond = (intent) =>
		intent.kind === 'remote' && intent.intent.kind === 'acquireSelected'
			? {
					kind: 'rejected',
					error: {
						category: 'validation',
						message: 'Select Audible titles before acquiring.',
						code: 'invalid_input',
						detail: null,
					},
				}
			: undefined;
	await owner.runAction({ type: 'acquireSelected' });
	expect(owner.view().statusMessage).toBe('Select Audible titles before acquiring.');
});
