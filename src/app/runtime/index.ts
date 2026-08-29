import { AtomRegistry } from './reactivity';
import { seedProcessing } from '../processing';
import { bindWorkOperationsRegistry, disposeWorkCenter } from '../workOperations';

export type AppRuntime = {
	readonly registry: AtomRegistry.AtomRegistry;
	dispose(): void;
};

export function createAppRuntime(): AppRuntime {
	const registry = AtomRegistry.make();
	seedProcessing(registry);
	bindWorkOperationsRegistry(registry);
	return {
		registry,
		dispose(): void {
			disposeWorkCenter();
			bindWorkOperationsRegistry(null);
			registry.dispose();
		},
	};
}
