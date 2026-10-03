import { afterEach, expect, it, vi } from 'vitest';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';
import type { AcquisitionJob } from '../../types/remoteSource';

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
async function open(
	engine: FakeEngine = createFakeEngine(),
	openAuthorizationUrl = vi.fn(async (_url: string) => undefined),
) {
	const runtime = createAppRuntime({ engine, remoteSource: { openAuthorizationUrl } });
	runtimes.push(runtime);
	await runtime.initialize();
	await runtime.remoteSource.open();
	return { runtime, engine, openAuthorizationUrl, owner: runtime.remoteSource };
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
		state.remote.lane = 'indexer';
	});
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
	engine.change((state) => {
		state.remote.lane = 'indexer';
	});
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
it('routes opening and lane selection only through the lane intent', async () => {
	const { owner, engine } = await open();
	await owner.selectLane('indexer');
	expect(engine.sessionIntents).toEqual([
		{ kind: 'remote', intent: { kind: 'selectLane', lane: 'audible' } },
		{ kind: 'remote', intent: { kind: 'selectLane', lane: 'indexer' } },
	]);
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

const title = {
	providerId: 'audible' as const,
	titleId: 'book-1',
	title: 'Restored book',
	authors: ['Writer'],
	narrators: [],
	supplementalPdfAvailable: false,
	acquired: false,
	availability: { status: 'available' as const, acquirable: true, label: 'Available' },
	unsupportedReasons: [],
};

it('restores cached library rows and selections during accepted acquisition without requesting a library load', async () => {
	const engine = createFakeEngine();
	engine.change((state) => {
		state.remote.account = { providerId: 'audible', status: 'connected' };
		state.remote.selectedTitleIds = [title.titleId];
		state.remote.acquisition = job;
		state.remoteLibrary.titles = [title];
	});
	const first = await open(engine);
	first.owner.close();
	first.runtime.dispose();
	const replacement = await open(engine);
	expect(replacement.owner.view().titles).toEqual([title]);
	expect([...replacement.owner.view().selectedTitleIds]).toEqual([title.titleId]);
	expect(engine.sessionIntents).toEqual([
		{ kind: 'remote', intent: { kind: 'selectLane', lane: 'audible' } },
		{ kind: 'remote', intent: { kind: 'selectLane', lane: 'audible' } },
	]);
});

it('routes authentication, library refresh, and disconnect intents and opens the initiating auth reply once', async () => {
	const { owner, engine, runtime, openAuthorizationUrl } = await open();
	const authorization = {
		providerId: 'audible' as const,
		authorizationUrl: 'https://auth.test',
		handoffPathHint: '/handoff',
		message: 'Authorize Audible',
	};
	engine.respond = (intent) =>
		intent.kind === 'remote' && intent.intent.kind === 'startAuth'
			? { kind: 'remoteAuthStarted', authorization }
			: undefined;
	await owner.runAction({ type: 'startAuth' });
	expect(openAuthorizationUrl).toHaveBeenCalledExactlyOnceWith('https://auth.test');
	owner.editSearch({ handoffPath: ' /handoff ' });
	await owner.runAction({ type: 'completeAuth' });
	await owner.runAction({ type: 'loadLibrary' });
	await owner.runAction({ type: 'logout' });
	expect(engine.sessionIntents.slice(1)).toEqual([
		{ kind: 'remote', intent: { kind: 'startAuth' } },
		{ kind: 'remote', intent: { kind: 'completeAuth', responseUrlHandoffPath: '/handoff' } },
		{ kind: 'remote', intent: { kind: 'refreshLibrary' } },
		{ kind: 'remote', intent: { kind: 'disconnect', provider: 'audible' } },
	]);
	engine.change((state) => {
		state.remote.auth = { kind: 'awaitingHandoff' };
	});
	runtime.dispose();
	const replacement = await open(engine, openAuthorizationUrl);
	expect(openAuthorizationUrl).toHaveBeenCalledTimes(1);
	expect(replacement.owner.view().statusMessage).toContain('authorization');
});

it.each(['dispose', 'reset'] as const)(
	'does not open a late authorization reply after %s',
	async (end) => {
		const { owner, runtime, engine, openAuthorizationUrl } = await open();
		const dispatch = engine.sessionDispatch.bind(engine);
		let finish!: () => void;
		engine.sessionDispatch = async (client, sequence, intent) => {
			if (intent.kind === 'remote' && intent.intent.kind === 'startAuth') {
				await new Promise<void>((resolve) => {
					finish = resolve;
				});
			}
			return dispatch(client, sequence, intent);
		};
		engine.respond = () => ({
			kind: 'remoteAuthStarted',
			authorization: {
				providerId: 'audible',
				authorizationUrl: 'https://auth.test',
				handoffPathHint: '',
				message: 'Authorize',
			},
		});
		const pending = owner.runAction({ type: 'startAuth' });
		await vi.waitFor(() => expect(finish).toBeDefined());
		if (end === 'dispose') runtime.dispose();
		else owner.reset();
		finish();
		await pending;
		expect(openAuthorizationUrl).not.toHaveBeenCalled();
	},
);

it('keeps Audible auth and library failures out of the Indexer lane', async () => {
	const { engine, owner } = await open();
	engine.change((state) => {
		state.remote.lane = 'indexer';
		state.remote.auth = {
			kind: 'failed',
			error: {
				category: 'resource',
				message: 'Audible auth failed',
				code: 'io_error',
				detail: null,
			},
		};
		state.remote.libraryStatus = { kind: 'running' };
		state.remote.indexer.message = 'Indexer ready';
	});
	expect(owner.view().statusMessage).toBe('Indexer ready');
	expect(owner.view().isBusy).toBe(false);
});

it('clears a previous frontend account-read failure when reopening retries the lane intent', async () => {
	const { owner, engine } = await open();
	engine.respond = () => ({
		kind: 'rejected',
		error: {
			category: 'io',
			code: 'io_error',
			message: 'Account read failed',
			detail: null,
		},
	});
	await owner.open();
	expect(owner.view().statusMessage).toBe('Account read failed');
	engine.respond = () => undefined;
	await owner.open();
	expect(owner.view().statusMessage).toBe('');
});
