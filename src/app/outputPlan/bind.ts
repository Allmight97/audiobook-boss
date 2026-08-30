import type { OutputDefaults } from '../../types/appSettings';
import type { OutputRequestConfig } from '../../types/audio';
import type { OutputPlanOwner } from './owner';
import { emptyOutputPlan, outputNamingFromPlan } from './types';

let boundOutput: OutputPlanOwner | undefined;

export function bindOutputOwner(output: OutputPlanOwner | undefined): void {
	boundOutput = output;
}

export function boundOutputOwner(): OutputPlanOwner | undefined {
	return boundOutput;
}

export function applyOutputDefaultsFromSettings(defaults: OutputDefaults): OutputDefaults {
	boundOutput?.applyDefaults(defaults);
	return defaults;
}

export function readOutputDefaultsFromState(): OutputDefaults {
	if (boundOutput) {
		return boundOutput.readDefaults();
	}
	const empty = emptyOutputPlan();
	return {
		outputDirectory: empty.outputDirectory || undefined,
		outputNaming: outputNamingFromPlan(empty),
	};
}

export function readOutputRequestConfig(): OutputRequestConfig {
	if (boundOutput) {
		return boundOutput.readRequestConfig();
	}
	throw new Error('Output directory not selected');
}

export function resetOutputPlanTimers(): void {}

export function resetOutputPlan(): void {
	resetOutputPlanTimers();
	boundOutput?.reset();
}
