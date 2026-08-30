import { Effect } from '../../lib/effect/appEffect';
import {
	liveSettingsCapability,
	type SettingsCapability,
} from '../../lib/tauri/capabilities/settings';
import type { ConcurrencyPreference } from '../../types/appSettings';
import type { MaxConcurrentJobsCapabilities } from '../../types/audio';
import { Atom } from '../runtime/reactivity';
import { resolveStartupDefaults } from './startupDefaults';

export type ConcurrencyView = {
	readonly selection: string;
	readonly effective: number | null;
	readonly effectiveLabel: string;
	readonly controlsEnabled: boolean;
	readonly allowAuto: boolean;
	readonly fixedOptions: ReadonlyArray<number>;
};

function emptyConcurrency(): ConcurrencyView {
	return {
		selection: 'auto',
		effective: null,
		effectiveLabel: '',
		controlsEnabled: true,
		allowAuto: true,
		fixedOptions: [],
	};
}

export const settingsCapabilityAtom = Atom.make<SettingsCapability>(liveSettingsCapability).pipe(
	Atom.keepAlive,
);

export const concurrencyViewAtom = Atom.make<ConcurrencyView>(emptyConcurrency()).pipe(
	Atom.keepAlive,
);

function labelFor(selection: string, effective: number | null): string {
	if (effective === null) return '';
	return selection === 'auto' ? `Auto → ${effective}` : `Max ${effective}`;
}

function preferenceFromSelection(value: string): ConcurrencyPreference {
	if (value === 'auto') return { mode: 'auto' };
	const parsed = Number.parseInt(value, 10);
	return { mode: 'fixed', value: parsed };
}

export const hydrateConcurrencyAtom = Atom.fn(
	(
		input: {
			readonly preference?: ConcurrencyPreference;
			readonly capabilities?: MaxConcurrentJobsCapabilities | null;
		},
		get,
	) => {
		const capability = get(settingsCapabilityAtom);
		return Effect.tryPromise({
			try: async () => {
				const runtime = await capability.getRuntimeSettingsCapabilities();
				const source = await resolveStartupDefaults(capability);
				const capabilities = input.capabilities ?? runtime.maxConcurrentJobs ?? null;
				const preference = input.preference ?? source.maxConcurrentJobs;
				const selection = preference.mode === 'fixed' ? String(preference.value) : 'auto';
				const effective =
					selection === 'auto'
						? await capability.setMaxConcurrentJobs(null)
						: await capability.setMaxConcurrentJobs(Number.parseInt(selection, 10));
				const latest = get(concurrencyViewAtom);
				get.set(concurrencyViewAtom, {
					...latest,
					selection,
					effective,
					allowAuto: capabilities?.allowAuto ?? true,
					fixedOptions: capabilities?.fixedOptions ?? [],
					effectiveLabel: labelFor(selection, effective),
				});
			},
			catch: (cause) => cause,
		}).pipe(
			Effect.catch((error) =>
				Effect.sync(() => {
					console.warn('Failed to hydrate max concurrency:', error);
				}),
			),
		);
	},
).pipe(Atom.keepAlive);

export const setConcurrencySelectionAtom = Atom.fn((value: string, get) => {
	const capability = get(settingsCapabilityAtom);
	const current = get(concurrencyViewAtom);
	const previous = current.selection;
	get.set(concurrencyViewAtom, { ...current, selection: value });
	return Effect.tryPromise({
		try: async () => {
			const preference = preferenceFromSelection(value);
			const settings = await capability.updateAppSettings({ maxConcurrentJobs: preference });
			const accepted = settings.maxConcurrentJobs;
			const selection = accepted.mode === 'fixed' ? String(accepted.value) : 'auto';
			let effective: number | null = null;
			try {
				effective = await capability.getMaxConcurrentJobs();
			} catch {
				effective = accepted.mode === 'fixed' ? accepted.value : null;
			}
			get.set(concurrencyViewAtom, {
				...get(concurrencyViewAtom),
				selection,
				effective,
				effectiveLabel: labelFor(selection, effective),
			});
		},
		catch: (cause) => cause,
	}).pipe(
		Effect.catch((error) =>
			Effect.sync(() => {
				console.warn('Failed to update max concurrency:', error);
				get.set(concurrencyViewAtom, { ...get(concurrencyViewAtom), selection: previous });
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const setConcurrencyControlsEnabledAtom = Atom.fnSync(
	(enabled: boolean, get) => {
		const current = get(concurrencyViewAtom);
		const next = { ...current, controlsEnabled: enabled };
		get.set(concurrencyViewAtom, next);
		return next;
	},
	{ initialValue: emptyConcurrency() },
).pipe(Atom.keepAlive);
