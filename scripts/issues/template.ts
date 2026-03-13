import type { DeliveryMode, HumanReviewMode, ParsedExecutionSpec } from './common';

export function renderReadyIssueBody(
	spec: ParsedExecutionSpec,
	options: { deliveryMode: DeliveryMode; humanReview: HumanReviewMode },
): string {
	return [
		'<!-- abb:issue-kind=ready -->',
		'',
		'## Goal',
		'',
		spec.goal,
		'',
		'## Constraints',
		'',
		spec.constraints,
		'',
		'## Acceptance',
		'',
		spec.acceptance,
		'',
		'## Validation',
		'',
		spec.validation,
		'',
		'## Delivery Mode',
		'',
		options.deliveryMode,
		'',
		'## Human Review',
		'',
		options.humanReview,
		'',
		'## Context',
		'',
		spec.context || 'None.',
		'',
	].join('\n');
}

export function renderIdeaIssueBody(content: string): string {
	const trimmed = content.trim();
	const body =
		trimmed.includes('\n## ') || trimmed.startsWith('## ')
			? trimmed
			: [
					'## Idea',
					'',
					trimmed || 'Describe the rough idea here.',
					'',
					'## Why It Might Matter',
					'',
					'',
					'## Open Questions',
					'',
					'',
					'## Notes',
					'',
					'',
				].join('\n');

	return ['<!-- abb:issue-kind=idea -->', '', body, ''].join('\n');
}
