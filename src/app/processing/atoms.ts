import { Effect } from '../../lib/effect/appEffect';
import { Atom, AtomRegistry } from '../runtime/reactivity';
import { bindProcessingRegistry } from './registry';
import {
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	triggerCancelAllFromStatusPanel,
	triggerProcessFromStatusPanel,
} from './runtime';
import { bindStatusViewRegistry, resetStatusPanelViewState, statusViewAtom } from './view';

export { statusViewAtom };
export type { StatusView } from './view';

export const startProcessingAtom = Atom.fn(
	(options: { previewSeconds?: number } | undefined) =>
		Effect.promise(() => initStatusPanel().startProcessing(options)),
).pipe(Atom.keepAlive);

export function seedProcessing(registry: AtomRegistry.AtomRegistry): void {
	bindProcessingRegistry(registry);
	bindStatusViewRegistry(registry);
	resetStatusPanelViewState();
	initStatusPanel();
}

export function resetProcessing(): void {
	resetStatusPanelViewState();
}

export {
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	triggerCancelAllFromStatusPanel,
	triggerProcessFromStatusPanel,
};
