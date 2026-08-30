import { Effect } from '../../lib/effect/appEffect';
import { Atom } from '../runtime/reactivity';
import { makeProductionLookupServices } from './services';
import {
	createMetadataLookupState,
	metadataLookupState,
	snapshotMetadataLookupState,
	type MetadataLookupApplyMode,
	type MetadataLookupSource,
	type MetadataLookupState,
} from './state';
import {
	makeMetadataLookupWorkflowServicesLayer,
	runMetadataLookupWorkflow,
	type MetadataLookupWorkflowAction,
} from './workflow';

export const lookupViewAtom = Atom.make<MetadataLookupState>(createMetadataLookupState()).pipe(
	Atom.keepAlive,
);

export const lookupPreviewRevisionAtom = Atom.make(0).pipe(Atom.keepAlive);

function publishLookupView(get: {
	readonly set: (atom: typeof lookupViewAtom, value: MetadataLookupState) => void;
}): void {
	get.set(lookupViewAtom, snapshotMetadataLookupState());
}

export const runLookupActionAtom = Atom.fn((action: MetadataLookupWorkflowAction, get) => {
	const layer = makeMetadataLookupWorkflowServicesLayer(
		makeProductionLookupServices(get as never, () => publishLookupView(get)),
	);
	return Effect.tryPromise({
		try: async () => {
			await runMetadataLookupWorkflow(layer, action);
			publishLookupView(get);
		},
		catch: (cause) => cause,
	}).pipe(
		Effect.catch((error) =>
			Effect.sync(() => {
				console.error('Metadata lookup failed:', error);
				publishLookupView(get);
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const setLookupTitleQueryAtom = Atom.fnSync((value: string, get) => {
	metadataLookupState.titleQuery = value;
	publishLookupView(get);
}).pipe(Atom.keepAlive);

export const setLookupAuthorQueryAtom = Atom.fnSync((value: string, get) => {
	metadataLookupState.authorQuery = value;
	publishLookupView(get);
}).pipe(Atom.keepAlive);

export const setLookupSourceAtom = Atom.fnSync((value: MetadataLookupSource, get) => {
	metadataLookupState.source = value;
	publishLookupView(get);
}).pipe(Atom.keepAlive);

export const setLookupApplyModeAtom = Atom.fnSync((value: MetadataLookupApplyMode, get) => {
	metadataLookupState.applyMode = value;
	publishLookupView(get);
}).pipe(Atom.keepAlive);

export const setLookupReplaceCoverArtAtom = Atom.fnSync((value: boolean, get) => {
	metadataLookupState.replaceCoverArt = value;
	publishLookupView(get);
}).pipe(Atom.keepAlive);

export const bumpLookupPreviewAtom = Atom.fnSync((_: undefined, get) => {
	get.set(lookupPreviewRevisionAtom, get(lookupPreviewRevisionAtom) + 1);
}).pipe(Atom.keepAlive);
