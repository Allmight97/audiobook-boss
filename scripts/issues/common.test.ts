import { describe, expect, test } from 'bun:test';

import { parseGitHubIssueTask, parseExecutionSpec } from './common';
import { renderReadyIssueBody } from './template';

describe('issues/common', () => {
	test('parses a ready GitHub issue contract', () => {
		const body = renderReadyIssueBody(
			{
				goal: 'Ship the change.',
				constraints: 'Keep it focused.',
				acceptance: '- [ ] It works.',
				validation: '- scripts/checks.sh standard',
				context: 'Extra notes.',
			},
			{ deliveryMode: 'pr', humanReview: 'visual' },
		);

		const issue = parseGitHubIssueTask({
			number: 42,
			title: 'Runner smoke',
			body,
			url: 'https://github.com/example/repo/issues/42',
			labels: ['enhancement'],
		});

		expect(issue.number).toBe(42);
		expect(issue.deliveryMode).toBe('pr');
		expect(issue.humanReview).toBe('visual');
		expect(issue.goal).toContain('Ship the change.');
	});

	test('rejects incomplete execution specs', () => {
		expect(() => parseExecutionSpec('## Goal\n\nShip it.\n')).toThrow(
			/Execution-ready issue content/,
		);
	});
});
