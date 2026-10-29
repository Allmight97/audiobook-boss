import { globSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = fileURLToPath(new URL('..', import.meta.url));
const stylesheets = Object.fromEntries(
	globSync('src/**/*.css', { cwd: root }).map((file) => [
		file,
		readFileSync(`${root}/${file}`, 'utf8'),
	]),
);

type Declaration = { readonly file: string; readonly text: string; readonly scopes: string[] };

function declarations(file: string, css: string): Declaration[] {
	const found: Declaration[] = [];
	const scopes: string[] = [];
	let buffer = '';
	for (const char of css.replace(/\/\*[\s\S]*?\*\//g, '')) {
		if (char === '{') {
			scopes.push(buffer.trim());
			buffer = '';
		} else if (char === '}') {
			if (buffer.trim()) found.push({ file, text: buffer.trim(), scopes: [...scopes] });
			scopes.pop();
			buffer = '';
		} else if (char === ';') {
			found.push({ file, text: buffer.trim(), scopes: [...scopes] });
			buffer = '';
		} else {
			buffer += char;
		}
	}
	return found;
}

const all = Object.entries(stylesheets).flatMap(([file, css]) => declarations(file, css));
const isTransition = (d: Declaration) => /^transition(-[a-z-]+)?\s*:/.test(d.text);
const namesProperties = (d: Declaration) => /^transition(-property)?\s*:/.test(d.text);
const ruleOf = (d: Declaration) => `${d.file}|${d.scopes.join('|')}`;
const allowsMotion = (d: Declaration) =>
	d.scopes.some((scope) => /prefers-reduced-motion:\s*no-preference/.test(scope));
const describeDeclaration = (d: Declaration) => `${d.file}: ${d.scopes.join(' > ')} { ${d.text} }`;

/**
 * Transitions that run under reduced motion or animate `all`, `width`, or
 * `height`. A longhand in a rule that names no properties animates `all`.
 */
function transitionProblems(list: readonly Declaration[]): string[] {
	const transitions = list.filter(isTransition);
	const named = new Set(transitions.filter(namesProperties).map(ruleOf));
	const unguarded = transitions.filter((d) => !allowsMotion(d));
	const broad = transitions.filter(
		(d) => (namesProperties(d) && /\b(all|width|height)\b/.test(d.text)) || !named.has(ruleOf(d)),
	);
	return [...new Set([...unguarded, ...broad])].map(describeDeclaration);
}

describe('motion policy', () => {
	it('runs an endless animation only when motion is allowed and its owner marks work active', () => {
		const endless = all.filter(
			(d) =>
				/^(-webkit-)?animation(-iteration-count)?\s*:/.test(d.text) && /\binfinite\b/.test(d.text),
		);
		expect(endless.length).toBeGreaterThan(0);
		const unguarded = endless.filter((d) => !allowsMotion(d));
		expect(unguarded.map(describeDeclaration)).toEqual([]);
		const gated = (selector: string) =>
			selector.replace(/:not\([^)]*\)/g, '').includes('[data-active]');
		const ungated = endless.filter((d) => {
			const rule = d.scopes.findLast((scope) => !scope.startsWith('@')) ?? '';
			return !rule.split(',').every(gated);
		});
		expect(ungated.map(describeDeclaration)).toEqual([]);
	});

	it('declares every transition only when motion is allowed, naming its properties', () => {
		expect(all.filter(isTransition).length).toBeGreaterThan(0);
		expect(transitionProblems(all)).toEqual([]);
	});

	it('catches a transition longhand that defaults to all outside the guard', () => {
		const probe = declarations('probe.css', '.probe { transition-duration: 0.2s; }');
		expect(transitionProblems(probe)).not.toEqual([]);
		const guarded = declarations(
			'probe.css',
			'@media (prefers-reduced-motion: no-preference) { .probe { transition-duration: 0.2s; } }',
		);
		expect(transitionProblems(guarded)).not.toEqual([]);
		const named = declarations(
			'probe.css',
			'@media (prefers-reduced-motion: no-preference) { .probe { transition-property: opacity; transition-duration: 0.2s; } }',
		);
		expect(transitionProblems(named)).toEqual([]);
	});
});
