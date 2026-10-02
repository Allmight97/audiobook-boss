import type { DisplayStage } from './state';
import type { JobStatus } from './state';

export type JobListItem = {
	key: string;
	label: string;
	status: JobStatus;
	statusText: string;
	stage?: DisplayStage;
	percentage?: number;
	canCancel: boolean;
	cancelId?: string;
	onCancel?: (id: string) => void;
};
