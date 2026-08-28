import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { inputCapabilityAtom, inputSessionAtom } from '../inputSession/atoms';
import { emptyInputSession } from '../inputSession/types';
import { createAppRuntime, type AppRuntime } from './index';

export function createTestAppRuntime(
	options: { readonly input?: InputCapability } = {},
): AppRuntime {
	const runtime = createAppRuntime();
	runtime.registry.set(inputSessionAtom, emptyInputSession());
	if (options.input) {
		runtime.registry.set(inputCapabilityAtom, options.input);
	}
	return runtime;
}
