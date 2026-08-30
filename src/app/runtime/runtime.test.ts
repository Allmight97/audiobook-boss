import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import { createAppRuntime } from './index';
import { emptyInputSession } from '../inputSession/types';
import { patchRemoteSourceState, snapshotRemoteSourceView } from '../remoteSource/state';
import {
	makeRemoteSourceWorkflowServicesLayer,
	runRemoteSourceWorkflow,
	type RemoteSourceWorkflowServices,
} from '../remoteSource/workflow';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

function runningJob(): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage: 10,
			message: 'Downloading audiobook.',
			terminal: false,
		},
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
	};
}

function remoteServices(status: Promise<AcquisitionJob>): RemoteSourceWorkflowServices {
	return {
		getAccountState: vi.fn(),
		startAuth: vi.fn(),
		openAuthorizationUrl: vi.fn(),
		completeAuth: vi.fn(),
		logout: vi.fn(),
		loadLibrary: vi.fn(),
		startAcquisition: vi.fn(async () => runningJob()),
		getAcquisitionStatus: vi.fn(() => status),
		cancelAcquisition: vi.fn(),
		purgeSession: vi.fn(),
		importMaterializedPaths: vi.fn(),
		sleep: vi.fn(async () => undefined),
	};
}

describe('app runtime', () => {
	let dispose: (() => void) | undefined;

	afterEach(() => {
		dispose?.();
		dispose = undefined;
	});

	it('disposes Solid session state so later runtimes do not share it', () => {
		const first = createAppRuntime();
		first.input.replaceSession({
			...emptyInputSession(),
			errorMessage: 'stale',
		});
		expect(first.input.view().errorMessage).toBe('stale');
		first.dispose();

		const second = createAppRuntime();
		dispose = () => second.dispose();
		expect(second.input.view().errorMessage).toBe('');
	});

	it('resets Remote Source module state on dispose so a remount does not keep the dialog open', () => {
		const runtime = createAppRuntime();
		patchRemoteSourceState({ isOpen: true, statusMessage: 'stale remote' });
		expect(snapshotRemoteSourceView().isOpen).toBe(true);
		runtime.dispose();
		expect(snapshotRemoteSourceView().isOpen).toBe(false);
		expect(snapshotRemoteSourceView().statusMessage).toBe('');
	});

	it('keeps late Remote Source polling from repopulating state after disposal', async () => {
		const runtime = createAppRuntime();
		const lateStatus = createDeferred<AcquisitionJob>();
		const services = remoteServices(lateStatus.promise);
		patchRemoteSourceState({ selectedTitleIds: new Set(['B000000001']) });
		const acquisition = runRemoteSourceWorkflow(makeRemoteSourceWorkflowServicesLayer(services), {
			type: 'acquireSelected',
		});
		await vi.waitFor(() => expect(services.getAcquisitionStatus).toHaveBeenCalledTimes(1));

		runtime.dispose();
		lateStatus.resolve(runningJob());
		await acquisition;

		expect(snapshotRemoteSourceView().activeJob).toBeNull();
		expect(snapshotRemoteSourceView().statusMessage).toBe('');
	});
});
