export type JobListItem = {
	key: string;
	label: string;
	statusText: string;
	percentage?: number;
	canCancel: boolean;
	cancelId?: string;
	onCancel?: (id: string) => void;
};
