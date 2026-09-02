import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import type { ProcessingPreflightPlan } from '../../types/audio';
import { createAppRuntime } from './index';
import { emptyInputSession } from '../inputSession/types';
import type { RemoteSourceWorkflowServices } from '../remoteSource/workflow';

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

function collisionPlan(): ProcessingPreflightPlan {
	return {
		jobType: 'batch',
		previewSeconds: undefined,
		collisionPolicy: 'fail',
		planSignature: 'sig-isolation',
		outputs: [
			{
				inputIndex: 0,
				inputPath: '/books/a.m4b',
				kind: 'final',
				requestedPath: '/tmp/out/a.m4b',
				resolvedPath: '/tmp/out/a.m4b',
				renameCandidate: undefined,
				collision: {
					kind: 'existing_file',
					conflictingPath: '/tmp/out/a.m4b',
					detail: 'An existing file already occupies the destination path.',
				},
				action: 'review_required',
			},
		],
	};
}

describe('app runtime', () => {
	let dispose: (() => void) | undefined;

	afterEach(() => {
		dispose?.();
		dispose = undefined;
	});

	it('does not share product-owner state across live runtimes', () => {
		const first = createAppRuntime();
		const second = createAppRuntime();
		dispose = () => {
			first.dispose();
			second.dispose();
		};

		void first.output.openCollisionReview(collisionPlan());
		void first.settings.openDialog();
		first.lookup.setTitleQuery('stale lookup');
		first.encoding.select('encoder', 'native_aac');
		first.processing.pushTransientStatus('first runtime only');
		first.remoteSource.patch({ isOpen: true, statusMessage: 'first runtime only' });

		expect(first.output.collision().isOpen).toBe(true);
		expect(second.output.collision().isOpen).toBe(false);
		expect(first.settings.dialog().isOpen).toBe(true);
		expect(second.settings.dialog().isOpen).toBe(false);
		expect(first.lookup.view().titleQuery).toBe('stale lookup');
		expect(second.lookup.view().titleQuery).toBe('');
		expect(first.encoding.request().encoderSettings.encoderType).toBe('native_aac');
		expect(second.encoding.request().encoderSettings.encoderType).toBe('auto');
		expect(first.processing.status().statusText).toBe('first runtime only');
		expect(second.processing.status().statusText).toBe('Idle');
		expect(first.remoteSource.view().isOpen).toBe(true);
		expect(second.remoteSource.view().isOpen).toBe(false);
		expect(second.remoteSource.view().statusMessage).toBe('');
		expect(first.workOperations.view().operations).toEqual([]);
		expect(second.workOperations.view().operations).toEqual([]);

		first.dispose();
		first.processing.pushTransientStatus('after dispose');
		expect(second.output.collision().isOpen).toBe(false);
		expect(second.settings.dialog().isOpen).toBe(false);
		expect(second.lookup.view().titleQuery).toBe('');
		expect(second.encoding.request().encoderSettings.encoderType).toBe('auto');
		expect(second.processing.status().statusText).toBe('Idle');
		expect(second.remoteSource.view().isOpen).toBe(false);

		const third = createAppRuntime();
		dispose = () => {
			second.dispose();
			third.dispose();
		};
		expect(third.output.collision().isOpen).toBe(false);
		expect(third.settings.dialog().isOpen).toBe(false);
		expect(third.lookup.view().titleQuery).toBe('');
		expect(third.encoding.request().encoderSettings.encoderType).toBe('auto');
		expect(third.processing.status().statusText).toBe('Idle');
		expect(third.remoteSource.view().isOpen).toBe(false);
		expect(third.workOperations.view().operations).toEqual([]);
	});

	it('isolates lookup and processing owners across disposed runtimes', () => {
		const first = createAppRuntime();
		first.lookup.setTitleQuery('stale lookup');
		expect(first.lookup.view().titleQuery).toBe('stale lookup');
		expect(first.processing.status().statusText).toBe('Idle');
		first.dispose();

		const second = createAppRuntime();
		dispose = () => second.dispose();
		expect(second.lookup.view().titleQuery).toBe('');
		expect(second.lookup.view().isOpen).toBe(false);
		expect(second.processing.status().isProcessing).toBe(false);
		expect(second.workOperations.view().operations).toEqual([]);
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
		runtime.remoteSource.patch({ isOpen: true, statusMessage: 'stale remote' });
		expect(runtime.remoteSource.view().isOpen).toBe(true);
		runtime.dispose();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		expect(runtime.remoteSource.view().statusMessage).toBe('');
	});

	it('keeps late Remote Source polling from repopulating state after disposal', async () => {
		const lateStatus = createDeferred<AcquisitionJob>();
		const services = remoteServices(lateStatus.promise);
		const runtime = createAppRuntime({ remoteSource: { services } });
		const other = createAppRuntime();
		dispose = () => other.dispose();
		other.remoteSource.patch({ isOpen: true, statusMessage: 'other runtime' });
		runtime.remoteSource.patch({ selectedTitleIds: new Set(['B000000001']) });
		const acquisition = runtime.remoteSource.runAction({
			type: 'acquireSelected',
		});
		await vi.waitFor(() => expect(services.getAcquisitionStatus).toHaveBeenCalledTimes(1));

		runtime.dispose();
		lateStatus.resolve(runningJob());
		await acquisition;

		expect(runtime.remoteSource.view().activeJob).toBeNull();
		expect(runtime.remoteSource.view().statusMessage).toBe('');
		expect(other.remoteSource.view().isOpen).toBe(true);
		expect(other.remoteSource.view().statusMessage).toBe('other runtime');
	});
});
