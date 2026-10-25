import { globSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// Enforces the motion rule in src/ui/AGENTS.md (Styles) across every app stylesheet.
const root = fileURLToPath(new URL('..', import.meta.url));
const stylesheets = Object.fromEntries(
	globSync('src/**/*.css', { cwd: root }).map((file) => [
		file,
		readFileSync(`${root}/${file}`, 'utf8'),
	]),
);

type Declaration = { readonly file: string; readonly text: string; readonly scopes: string[] };

/** Every declaration with the at-rule and selector preludes that enclose it. */
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
const describeDeclaration = (d: Declaration) => `${d.file}: ${d.scopes.join(' > ')} { ${d.text} }`;

describe('motion policy', () => {
	it('runs an endless animation only when motion is allowed and its owner marks work active', () => {
		const endless = all.filter(
			(d) =>
				/^(-webkit-)?animation(-iteration-count)?\s*:/.test(d.text) && /\binfinite\b/.test(d.text),
		);
		expect(endless.length).toBeGreaterThan(0);
		const unguarded = endless.filter(
			(d) => !d.scopes.some((scope) => /prefers-reduced-motion:\s*no-preference/.test(scope)),
		);
		expect(unguarded.map(describeDeclaration)).toEqual([]);
		const gated = (selector: string) =>
			selector.replace(/:not\([^)]*\)/g, '').includes('[data-active]');
		const ungated = endless.filter((d) => {
			const rule = d.scopes.findLast((scope) => !scope.startsWith('@')) ?? '';
			return !rule.split(',').every(gated);
		});
		expect(ungated.map(describeDeclaration)).toEqual([]);
	});

	it('never transitions width or height', () => {
		const layout = all.filter(
			(d) => /^transition(-property)?\s*:/.test(d.text) && /\b(width|height)\b/.test(d.text),
		);
		expect(layout.map(describeDeclaration)).toEqual([]);
	});
});
