import type { EventStage } from '../../types/events';
import type { JobStatus } from './state';

export type JobListItem = {
	key: string;
	label: string;
	status: JobStatus;
	statusText: string;
	stage?: EventStage;
	percentage?: number;
	canCancel: boolean;
	cancelId?: string;
	onCancel?: (id: string) => void;
};
