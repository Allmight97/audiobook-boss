import { readFile } from 'node:fs/promises';

export type DeliveryMode = 'pr' | 'no-pr';
export type HumanReviewMode = 'none' | 'visual';

export type ParsedExecutionSpec = {
	goal: string;
	constraints: string;
	acceptance: string;
	validation: string;
	context: string;
};

export type GitHubIssueTask = ParsedExecutionSpec & {
	number: number;
	title: string;
	url: string;
	labels: string[];
	deliveryMode: DeliveryMode;
	humanReview: HumanReviewMode;
	runKey: string;
};

export function getSectionMap(body: string): Record<string, string> {
	const map: Record<string, string> = {};
	let activeKey: string | null = null;
	const buffer: string[] = [];

	const flush = () => {
		if (!activeKey) return;
		map[activeKey] = buffer.join('\n').trim();
		buffer.length = 0;
	};

	for (const line of body.split('\n')) {
		if (line.startsWith('## ')) {
			flush();
			activeKey = line.slice(3).trim().toLowerCase();
			continue;
		}
		if (activeKey) {
			buffer.push(line);
		}
	}

	flush();
	return map;
}

function normalizeMode<T extends string>(
	value: string | undefined,
	allowed: readonly T[],
	fieldName: string,
): T {
	const normalized = value?.trim().toLowerCase() as T | undefined;
	if (!normalized || !allowed.includes(normalized)) {
		throw new Error(
			`Issue is missing a valid ${fieldName}. Expected one of: ${allowed.join(', ')}.`,
		);
	}
	return normalized;
}

export function parseExecutionSpec(raw: string): ParsedExecutionSpec {
	const sections = getSectionMap(raw);
	const goal = sections.goal?.trim() ?? '';
	const constraints = sections.constraints?.trim() ?? '';
	const acceptance = sections.acceptance?.trim() ?? '';
	const validation = sections.validation?.trim() ?? '';
	const context = sections.context?.trim() ?? '';

	if (!goal || !constraints || !acceptance || !validation) {
		throw new Error(
			'Execution-ready issue content must include ## Goal, ## Constraints, ## Acceptance, and ## Validation.',
		);
	}

	return { goal, constraints, acceptance, validation, context };
}

function stripMarker(body: string): string {
	return body.replace(/<!--\s*abb:issue-kind=[^>]+-->\s*/i, '').trim();
}

export function parseGitHubIssueTask(input: {
	number: number;
	title: string;
	body: string;
	url: string;
	labels: string[];
}): GitHubIssueTask {
	const marker = input.body.match(/<!--\s*abb:issue-kind=([a-z-]+)\s*-->/i)?.[1]?.toLowerCase();
	if (marker !== 'ready') {
		throw new Error(
			`Issue #${input.number} is not execution-ready. Use the ready issue template before running it.`,
		);
	}

	const sections = getSectionMap(stripMarker(input.body));
	const spec = parseExecutionSpec(stripMarker(input.body));
	const deliveryMode = normalizeMode(sections['delivery mode'], ['pr', 'no-pr'], 'Delivery Mode');
	const humanReview = normalizeMode(sections['human review'], ['none', 'visual'], 'Human Review');

	return {
		number: input.number,
		title: input.title,
		url: input.url,
		labels: input.labels.map((label) => label.toLowerCase()),
		deliveryMode,
		humanReview,
		runKey: `issue-${input.number}`,
		...spec,
	};
}

export async function readExecutionSpecFile(filePath: string): Promise<ParsedExecutionSpec> {
	return parseExecutionSpec(await readFile(filePath, 'utf8'));
}
