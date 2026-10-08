import { describe, expect, it } from 'vitest';
import generatedBindings from '../lib/generated/tauri.ts?raw';

const TEST_FILE = 'src/__tests__/session-intent.contract.test.ts';

const POSITION_INTEGER_MESSAGE = (variant: string, field: string) =>
	`SessionIntent::${variant}.${field} is an integer. If it addresses a title, send title_id: String and resolve it inside the session with WorkingSet::index_of. If it is a slot or in-title order, add it to the allowlist in ${TEST_FILE} with a reason.`;

type IntegerField = {
	readonly variant: string;
	readonly field: string;
};

type AllowedIntegerField = IntegerField & { readonly reason: string };

// Generated TypeScript types usize, u64, and f64 as `number`. Entries the
// Architect named are the remaining title-adjacent integers; the rest are
// identities and durations, not list positions.
const ALLOWED_INTEGER_FIELDS: readonly AllowedIntegerField[] = [
	{ variant: 'ReorderFiles', field: 'to', reason: 'destination slot.' },
	{
		variant: 'ReorderSources',
		field: 'from',
		reason: 'source order inside a title already named by title_id.',
	},
	{
		variant: 'ReorderSources',
		field: 'to',
		reason: 'source order inside a title already named by title_id.',
	},
	{ variant: 'LookupApply', field: 'index', reason: 'search-result index.' },
	{ variant: 'Preview', field: 'seconds', reason: 'preview duration, not a title position.' },
	{
		variant: 'ChooseCollisionPolicy',
		field: 'reviewId',
		reason: 'names a held collision review.',
	},
	{
		variant: 'CancelCollisionReview',
		field: 'reviewId',
		reason: 'names a held collision review.',
	},
	{
		variant: 'RestartTitle',
		field: 'revision',
		reason: 'names a restart offer revision.',
	},
	{
		variant: 'KeepTitleLocation',
		field: 'revision',
		reason: 'names a restart offer revision.',
	},
];

function sessionIntentAlias(source: string): string {
	const start = source.indexOf('export type SessionIntent =');
	if (start < 0) {
		throw new Error('SessionIntent type missing from generated bindings');
	}
	const from = start + 'export type SessionIntent ='.length;
	const nextExport = source.indexOf('\nexport type ', from);
	if (nextExport < 0) {
		throw new Error('SessionIntent type has no following export');
	}
	return source.slice(from, nextExport);
}

function stripComments(source: string): string {
	return source.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/\/\/.*$/gm, ' ');
}

function splitTopLevel(source: string, separator: string): string[] {
	const parts: string[] = [];
	let depth = 0;
	let start = 0;
	for (let index = 0; index < source.length; index += 1) {
		const char = source[index];
		if (char === '{') depth += 1;
		if (char === '}') depth -= 1;
		if (depth !== 0 || char !== separator) continue;
		parts.push(source.slice(start, index));
		start = index + separator.length;
	}
	parts.push(source.slice(start));
	return parts.map((part) => part.trim()).filter(Boolean);
}

function parseVariant(member: string): {
	kind: string;
	fields: Array<{ name: string; type: string }>;
} {
	const body = member.replace(/^\s*\{/, '').replace(/\}\s*;?\s*$/, '');
	const fields = splitTopLevel(body, ';').map((entry) => {
		const colon = entry.indexOf(':');
		return {
			name: entry.slice(0, colon).trim(),
			type: entry.slice(colon + 1).trim(),
		};
	});
	const kindField = fields.find((field) => field.name === 'kind');
	const kind = kindField?.type.match(/^"(.+)"$/)?.[1];
	if (!kind) {
		throw new Error(`SessionIntent member has no kind: ${member}`);
	}
	return { kind, fields: fields.filter((field) => field.name !== 'kind') };
}

function isIntegerType(type: string): boolean {
	const compact = type.replace(/\s+/g, '');
	return compact === 'number' || compact === 'number|null' || compact === 'null|number';
}

function kindToVariant(kind: string): string {
	return `${kind.slice(0, 1).toUpperCase()}${kind.slice(1)}`;
}

function integerFields(source: string): IntegerField[] {
	return splitTopLevel(stripComments(sessionIntentAlias(source)), '|').flatMap((member) => {
		const variant = parseVariant(member);
		const name = kindToVariant(variant.kind);
		return variant.fields
			.filter((field) => isIntegerType(field.type))
			.map((field) => ({ variant: name, field: field.name }));
	});
}

function allowlistKey(field: IntegerField): string {
	return `${field.variant}.${field.field}`;
}

describe('SessionIntent binding schema', () => {
	it('allowlists every integer field and rejects a title addressed by position', () => {
		const found = integerFields(generatedBindings);
		const allowed = new Set(ALLOWED_INTEGER_FIELDS.map(allowlistKey));
		for (const entry of ALLOWED_INTEGER_FIELDS) {
			expect(
				found.some((field) => allowlistKey(field) === allowlistKey(entry)),
				`parser missed allowlisted ${entry.variant}.${entry.field} (${entry.reason})`,
			).toBe(true);
		}
		const unexpected = found.filter((field) => !allowed.has(allowlistKey(field)));
		if (unexpected[0]) {
			throw new Error(POSITION_INTEGER_MESSAGE(unexpected[0].variant, unexpected[0].field));
		}
	});
});
