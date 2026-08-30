import type { OutputPlanOwner } from './owner';

let boundOutput: OutputPlanOwner | undefined;

export function bindOutputOwner(output: OutputPlanOwner | undefined): void {
	boundOutput = output;
}

export function boundOutputOwner(): OutputPlanOwner | undefined {
	return boundOutput;
}
