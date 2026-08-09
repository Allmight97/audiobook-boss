export type BoundedGenerationQueueSchedule<T> = {
	prepare: (key: string, generation: number) => boolean;
	start: (key: string, generation: number, complete: () => void) => Promise<T>;
	visibleKeysChanged?: (visibleKeys: ReadonlySet<string>, generation: number) => void;
};

export type BoundedGenerationQueue = {
	cancel: () => void;
	currentGeneration: () => number;
	isCurrent: (key: string, generation: number) => boolean;
	schedule: <T>(keys: ReadonlyArray<string>, options: BoundedGenerationQueueSchedule<T>) => void;
	track: <T>(task: Promise<T>) => Promise<T>;
};

/** Bounds active work while allowing stale requests to finish without committing. */
export function createBoundedGenerationQueue(concurrency: number): BoundedGenerationQueue {
	let generation = 0;
	let visibleKeys = new Set<string>();
	let queuedKeys: string[] = [];
	let activeCount = 0;
	let activeSchedule: BoundedGenerationQueueSchedule<unknown> | null = null;

	function cancel(): void {
		generation += 1;
		visibleKeys = new Set();
		queuedKeys = [];
		activeSchedule = null;
	}
	function currentGeneration(): number {
		return generation;
	}
	function isCurrent(key: string, expectedGeneration: number): boolean {
		return expectedGeneration === generation && visibleKeys.has(key);
	}
	function track<T>(task: Promise<T>): Promise<T> {
		activeCount += 1;
		let completed = false;
		const complete = () => {
			if (completed) return;
			completed = true;
			activeCount -= 1;
			pump();
		};
		void task.then(complete, complete);
		return task;
	}
	function schedule<T>(
		keys: ReadonlyArray<string>,
		options: BoundedGenerationQueueSchedule<T>,
	): void {
		generation += 1;
		const scheduleGeneration = generation;
		const uniqueKeys = [...new Set(keys)];
		visibleKeys = new Set(uniqueKeys);
		queuedKeys = [];
		activeSchedule = options as BoundedGenerationQueueSchedule<unknown>;
		options.visibleKeysChanged?.(visibleKeys, scheduleGeneration);
		for (const key of uniqueKeys)
			if (options.prepare(key, scheduleGeneration)) queuedKeys.push(key);
		pump();
	}
	function pump(): void {
		const schedule = activeSchedule;
		if (!schedule) return;
		while (activeCount < concurrency && queuedKeys.length > 0) {
			const key = queuedKeys.shift();
			if (!key || !isCurrent(key, generation)) continue;
			activeCount += 1;
			let completed = false;
			const complete = () => {
				if (completed) return;
				completed = true;
				activeCount -= 1;
				pump();
			};
			let task: Promise<unknown>;
			try {
				task = schedule.start(key, generation, complete);
			} catch (error) {
				task = Promise.reject(error);
			}
			void task.then(complete, complete);
		}
	}
	return { cancel, currentGeneration, isCurrent, schedule, track };
}
