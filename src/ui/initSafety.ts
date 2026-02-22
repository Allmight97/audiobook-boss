type InitStep = {
	label: string;
	init: () => void;
};

export function runInitSteps(steps: readonly InitStep[]): void {
	for (const step of steps) {
		try {
			step.init();
		} catch (error) {
			console.error(`[ui:init] ${step.label} failed`, error);
			throw new Error(`[ui:init] ${step.label} failed: ${String(error)}`);
		}
	}
}
