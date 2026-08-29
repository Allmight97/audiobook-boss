import { Effect } from '../../lib/effect/appEffect';
import { Atom } from '../runtime/reactivity';
import { inputViewAtom } from '../inputSession';
import { clearRemoteSourceCoverPreviewCache } from './coverPreview';
import { makeProductionRemoteSourceServices } from './services';
import {
	reconcileRemoteSourceSessionsWithInput,
	resetRemoteSourceSessionAssets,
} from './sessionAssets';
import { patchRemoteSourceState, resetRemoteSourceState, snapshotRemoteSourceView } from './state';
import {
	makeRemoteSourceWorkflowServicesLayer,
	runRemoteSourceWorkflow,
	type RemoteSourceWorkflowAction,
} from './workflow';

export const remoteSourceViewAtom = Atom.make(snapshotRemoteSourceView()).pipe(Atom.keepAlive);

export const remoteSourceLifetimeAtom = Atom.make((get) => {
	const files = get(inputViewAtom).files;
	void reconcileRemoteSourceSessionsWithInput(files);
	return files.map((file) => file.inputId ?? file.path);
}).pipe(Atom.keepAlive);

function publishRemoteSourceView(get: {
	readonly set: (
		atom: typeof remoteSourceViewAtom,
		value: ReturnType<typeof snapshotRemoteSourceView>,
	) => void;
}): void {
	get.set(remoteSourceViewAtom, snapshotRemoteSourceView());
}

export const runRemoteSourceActionAtom = Atom.fn((action: RemoteSourceWorkflowAction, get) => {
	const layer = makeRemoteSourceWorkflowServicesLayer(
		makeProductionRemoteSourceServices(get as never),
	);
	return Effect.tryPromise({
		try: async () => {
			await runRemoteSourceWorkflow(layer, action);
			publishRemoteSourceView(get);
		},
		catch: (cause) => cause,
	}).pipe(
		Effect.catch((error) =>
			Effect.sync(() => {
				console.error('Remote source workflow failed:', error);
				publishRemoteSourceView(get);
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const openRemoteSourceAcquireAtom = Atom.fnSync((_: undefined, get) => {
	patchRemoteSourceState({ isOpen: true });
	publishRemoteSourceView(get);
}).pipe(Atom.keepAlive);

export const closeRemoteSourceAcquireAtom = Atom.fnSync((_: undefined, get) => {
	patchRemoteSourceState({ isOpen: false, didHydrateOpenDialog: false });
	publishRemoteSourceView(get);
}).pipe(Atom.keepAlive);

export const patchRemoteSourceViewAtom = Atom.fnSync(
	(patch: Parameters<typeof patchRemoteSourceState>[0], get) => {
		patchRemoteSourceState(patch);
		publishRemoteSourceView(get);
	},
).pipe(Atom.keepAlive);

export function resetRemoteSource(): void {
	resetRemoteSourceState();
	resetRemoteSourceSessionAssets();
	clearRemoteSourceCoverPreviewCache();
}
