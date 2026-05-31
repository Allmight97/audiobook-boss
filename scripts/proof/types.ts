export type ProofClassification = 'focused' | 'review' | 'release' | 'diagnostic';

export type ProofStep = {
	id: string;
	label: string;
	tool: 'bash' | 'bun' | 'cargo';
	command: string;
	args: string[];
	preflight?: {
		args: string[];
		command: string;
		hint: string;
	};
	reportOnSuccess?: boolean;
	requiredEnv?: string[];
};

export type ProofPlan = {
	id: string;
	label: string;
	classification: ProofClassification;
	purpose: string;
	steps: ProofStep[];
};

export type ProofStepResult = ProofStep & {
	durationMs: number;
	exitCode: number | null;
	logPath: string;
	status: 'failed' | 'passed';
};

export type ProofSummary = {
	artifactDir: string;
	durationMs: number;
	failedStepId?: string;
	plan: {
		classification: ProofClassification;
		id: string;
		label: string;
		purpose: string;
	};
	status: 'failed' | 'passed';
	steps: ProofStepResult[];
};

export type ProofEvent =
	| {
			artifactDir: string;
			kind: 'run_started';
			planId: string;
			timestamp: string;
	  }
	| {
			command: string[];
			kind: 'step_started';
			requiredEnv?: string[];
			stepId: string;
			timestamp: string;
	  }
	| {
			durationMs: number;
			exitCode: number | null;
			kind: 'step_finished';
			logPath: string;
			status: 'failed' | 'passed';
			stepId: string;
			timestamp: string;
	  }
	| {
			kind: 'run_finished';
			status: 'failed' | 'passed';
			timestamp: string;
	  }
	| {
			kind: 'artifact_written';
			path: string;
			timestamp: string;
	  }
	| {
			kind: 'next_action_hint';
			message: string;
			timestamp: string;
	  };
