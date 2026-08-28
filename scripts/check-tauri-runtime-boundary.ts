import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const repoRoot = process.cwd();
const generatedModulePath = path.join(repoRoot, 'src/lib/generated/tauri');
const allowedCommandImporter = path.join(repoRoot, 'src/lib/tauri/commands.ts');
const allowedEventImporter = path.join(repoRoot, 'src/lib/tauri/client.ts');
const tauriBoundaryDir = withTrailingSeparator(path.join(repoRoot, 'src/lib/tauri'));
const testSupportDir = withTrailingSeparator(path.join(repoRoot, 'src/test'));

type Violation = {
	file: string;
	line: number;
	message: string;
};

type NamedBinding = {
	name: string;
	typeOnly: boolean;
};

type ModuleStatement = {
	kind: 'import' | 'export';
	index: number;
	typeOnly: boolean;
	hasDefault: boolean;
	hasNamespace: boolean;
	names: NamedBinding[];
	source: string;
};

const sourceFiles = collectSourceFiles(path.join(repoRoot, 'src'));
const violations: Violation[] = [];

for (const file of sourceFiles) {
	if (normalizeFilePath(file) === normalizeFilePath(`${generatedModulePath}.ts`)) {
		continue;
	}
	checkFile(file);
}

if (violations.length > 0) {
	for (const violation of violations) {
		console.error(`${violation.file}:${violation.line}: ${violation.message}`);
	}
	process.exit(1);
}

console.log('[check-tauri-runtime-boundary] OK');

function collectSourceFiles(root: string): string[] {
	const files: string[] = [];
	for (const entry of readdirSync(root)) {
		const fullPath = path.join(root, entry);
		const stat = statSync(fullPath);
		if (stat.isDirectory()) {
			files.push(...collectSourceFiles(fullPath));
		} else if (/\.(ts|svelte)$/.test(entry)) {
			files.push(fullPath);
		}
	}
	return files;
}

function checkFile(file: string): void {
	const content = readFileSync(file, 'utf8');
	for (const block of sourceBlocks(file, content)) {
		checkSourceBlock(file, content, block);
	}
}

function checkSourceBlock(file: string, fullContent: string, block: SourceBlock): void {
	const code = blankComments(block.content);
	for (const statement of parseModuleStatements(code)) {
		checkModuleStatement(file, fullContent, block, statement);
	}
	if (!allowsRawTauriCore(file)) {
		checkRawTauriInvokeUsage(file, fullContent, block, code);
	}
}

function checkModuleStatement(
	file: string,
	fullContent: string,
	block: SourceBlock,
	statement: ModuleStatement,
): void {
	const line = lineNumberAt(fullContent, block.start + statement.index);
	if (statement.source === '@tauri-apps/api/core' && !allowsRawTauriCore(file)) {
		pushRawTauriCoreImportViolations(file, line, statement);
		return;
	}

	if (statement.typeOnly || !isGeneratedTauriImport(file, statement.source)) {
		return;
	}

	pushNamedValueViolations(
		file,
		line,
		new Set(statement.names.filter((binding) => !binding.typeOnly).map((binding) => binding.name)),
	);

	if (statement.kind === 'export' && (statement.hasNamespace || statement.names.length === 0)) {
		violations.push({
			file: displayPath(file),
			line,
			message:
				'generated Tauri value re-exports are not allowed; import commands or events through the owning boundary file',
		});
		return;
	}

	if (statement.hasNamespace) {
		violations.push({
			file: displayPath(file),
			line,
			message:
				'generated Tauri namespace value imports are not allowed; import commands or events through the owning boundary file',
		});
	}
}

function checkRawTauriInvokeUsage(
	file: string,
	fullContent: string,
	block: SourceBlock,
	code: string,
): void {
	const searchable = blankQuoted(code);
	for (const match of searchable.matchAll(/\b__TAURI_INVOKE\b/g)) {
		violations.push({
			file: displayPath(file),
			line: lineNumberAt(fullContent, block.start + (match.index ?? 0)),
			message: 'raw __TAURI_INVOKE usage must stay out of runtime app code; use tauriClient',
		});
	}
}

function pushRawTauriCoreImportViolations(
	file: string,
	line: number,
	statement: ModuleStatement,
): void {
	if (statement.typeOnly) {
		return;
	}
	if (statement.hasDefault) {
		violations.push({
			file: displayPath(file),
			line,
			message: 'raw Tauri core default imports must stay out of runtime app code; use tauriClient',
		});
	}
	if (statement.hasNamespace) {
		violations.push({
			file: displayPath(file),
			line,
			message:
				'raw Tauri core namespace imports must stay out of runtime app code; use tauriClient',
		});
		return;
	}
	if (statement.names.some((binding) => !binding.typeOnly && binding.name === 'invoke')) {
		violations.push({
			file: displayPath(file),
			line,
			message: "raw Tauri 'invoke' imports must stay out of runtime app code; use tauriClient",
		});
	}
}

type SourceBlock = {
	content: string;
	start: number;
};

function sourceBlocks(file: string, content: string): SourceBlock[] {
	if (!file.endsWith('.svelte')) {
		return [{ content, start: 0 }];
	}

	const blocks: SourceBlock[] = [];
	for (const match of content.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)) {
		const script = match[1] ?? '';
		const scriptStart = (match.index ?? 0) + match[0].indexOf(script);
		blocks.push({ content: script, start: scriptStart });
	}
	return blocks;
}

function parseModuleStatements(source: string): ModuleStatement[] {
	const statements: ModuleStatement[] = [];
	const keyword = /\b(import|export)\b/g;
	let match = keyword.exec(source);
	while (match) {
		const kind = match[1] as 'import' | 'export';
		const start = match.index;
		if (kind === 'import' && source[start + 6] === '(') {
			match = keyword.exec(source);
			continue;
		}
		const parsed = parseModuleStatement(kind, source.slice(start));
		if (!parsed) {
			match = keyword.exec(source);
			continue;
		}
		const { consumed, ...statement } = parsed;
		statements.push({ ...statement, kind, index: start });
		keyword.lastIndex = start + consumed;
		match = keyword.exec(source);
	}
	return statements;
}

function parseModuleStatement(
	kind: 'import' | 'export',
	text: string,
): (Omit<ModuleStatement, 'kind' | 'index'> & { consumed: number }) | null {
	const prefix = kind === 'import' ? /^import\s+/ : /^export\s+/;
	const afterKeyword = text.match(prefix);
	if (!afterKeyword) {
		return null;
	}

	let cursor = afterKeyword[0].length;
	const typeOnly = /^(type\s+)(?!as\b)/.test(text.slice(cursor));
	if (typeOnly) {
		cursor += text.slice(cursor).match(/^type\s+/)?.[0].length ?? 0;
	}

	const fromMatch = findFromClause(text.slice(cursor));
	if (!fromMatch) {
		return null;
	}

	const bindings = text.slice(cursor, cursor + fromMatch.bindingsEnd).trim();
	const source = fromMatch.source;
	const consumed = cursor + fromMatch.consumed;
	if (bindings === '*') {
		return {
			typeOnly,
			hasDefault: false,
			hasNamespace: true,
			names: [],
			source,
			consumed,
		};
	}

	return {
		typeOnly,
		...parseBindings(bindings),
		source,
		consumed,
	};
}

function findFromClause(
	text: string,
): { bindingsEnd: number; source: string; consumed: number } | null {
	let depth = 0;
	for (let index = 0; index < text.length; index += 1) {
		const character = text[index];
		if (character === '{' || character === '(') {
			depth += 1;
			continue;
		}
		if (character === '}' || character === ')') {
			depth = Math.max(0, depth - 1);
			continue;
		}
		if (depth !== 0 || !text.startsWith('from', index) || /\S/.test(text[index - 1] ?? ' ')) {
			continue;
		}
		const specifier = text.slice(index + 4).match(/^\s*(['"])([^'"]+)\1/);
		if (!specifier) {
			return null;
		}
		return {
			bindingsEnd: index,
			source: specifier[2] ?? '',
			consumed: index + 4 + specifier[0].length,
		};
	}
	return null;
}

function parseBindings(
	bindings: string,
): Pick<ModuleStatement, 'hasDefault' | 'hasNamespace' | 'names'> {
	const hasNamespace = /(?:^|,)\s*\*(?:\s+as\s+[A-Za-z_$][\w$]*)?/.test(bindings);
	const hasDefault = /^[A-Za-z_$][\w$]*\s*(,|$)/.test(bindings);
	const named = bindings.match(/\{([\s\S]*)\}/);
	if (!named) {
		return { hasDefault, hasNamespace, names: [] };
	}

	const names: NamedBinding[] = [];
	for (const raw of splitBindingList(named[1] ?? '')) {
		const specifier = raw.trim();
		if (!specifier) {
			continue;
		}
		const typeOnly = /^type\s+/.test(specifier);
		const rest = typeOnly ? specifier.slice(5).trim() : specifier;
		const name = rest.split(/\s+as\s+/)[0]?.trim();
		if (name) {
			names.push({ name, typeOnly });
		}
	}
	return { hasDefault, hasNamespace, names };
}

function splitBindingList(list: string): string[] {
	const parts: string[] = [];
	let current = '';
	let depth = 0;
	for (const character of list) {
		if (character === '{' || character === '(') {
			depth += 1;
		} else if (character === '}' || character === ')') {
			depth = Math.max(0, depth - 1);
		}
		if (character === ',' && depth === 0) {
			parts.push(current);
			current = '';
			continue;
		}
		current += character;
	}
	if (current.trim()) {
		parts.push(current);
	}
	return parts;
}

function blankComments(source: string): string {
	let output = '';
	let index = 0;
	let quote: "'" | '"' | '`' | null = null;
	while (index < source.length) {
		const character = source[index] ?? '';
		const next = source[index + 1] ?? '';
		if (quote) {
			output += character;
			if (character === '\\' && quote !== '`') {
				output += next;
				index += 2;
				continue;
			}
			if (character === quote) {
				quote = null;
			}
			index += 1;
			continue;
		}
		if (character === '/' && next === '/') {
			const end = source.indexOf('\n', index);
			const length = (end === -1 ? source.length : end) - index;
			output += ' '.repeat(length);
			index += length;
			continue;
		}
		if (character === '/' && next === '*') {
			const end = source.indexOf('*/', index + 2);
			const close = end === -1 ? source.length : end + 2;
			output += source.slice(index, close).replace(/[^\n]/g, ' ');
			index = close;
			continue;
		}
		if (character === "'" || character === '"' || character === '`') {
			quote = character;
		}
		output += character;
		index += 1;
	}
	return output;
}

function blankQuoted(source: string): string {
	let output = '';
	let index = 0;
	let quote: "'" | '"' | '`' | null = null;
	while (index < source.length) {
		const character = source[index] ?? '';
		if (quote) {
			if (character === '\\' && quote !== '`') {
				output += '  ';
				index += 2;
				continue;
			}
			if (character === quote) {
				output += character;
				quote = null;
				index += 1;
				continue;
			}
			output += character === '\n' ? '\n' : ' ';
			index += 1;
			continue;
		}
		if (character === "'" || character === '"' || character === '`') {
			quote = character;
			output += character;
			index += 1;
			continue;
		}
		output += character;
		index += 1;
	}
	return output;
}

function pushNamedValueViolations(file: string, line: number, names: Set<string>): void {
	if (
		names.has('commands') &&
		normalizeFilePath(file) !== normalizeFilePath(allowedCommandImporter)
	) {
		violations.push({
			file: displayPath(file),
			line,
			message: "generated 'commands' value imports must stay inside src/lib/tauri/commands.ts",
		});
	}

	if (names.has('events') && normalizeFilePath(file) !== normalizeFilePath(allowedEventImporter)) {
		violations.push({
			file: displayPath(file),
			line,
			message: "generated 'events' value imports must stay inside src/lib/tauri/client.ts",
		});
	}
}

function isGeneratedTauriImport(file: string, source: string): boolean {
	const resolved = resolveImportPath(file, source);
	if (!resolved) {
		return false;
	}
	return normalizeFilePath(resolved) === normalizeFilePath(generatedModulePath);
}

function resolveImportPath(file: string, source: string): string | null {
	if (source.startsWith('.')) {
		return stripKnownExtension(path.resolve(path.dirname(file), source));
	}
	if (source.startsWith('src/')) {
		return stripKnownExtension(path.resolve(repoRoot, source));
	}
	return null;
}

function stripKnownExtension(value: string): string {
	return value.replace(/\.(ts|js|svelte)$/, '');
}

function allowsRawTauriCore(file: string): boolean {
	const normalized = normalizeFilePath(file);
	if (normalized.startsWith(tauriBoundaryDir) || normalized.startsWith(testSupportDir)) {
		return true;
	}
	if (/[./](test|spec)\.ts$/.test(normalized)) {
		return true;
	}
	return normalized.includes(`${path.sep}__tests__${path.sep}`);
}

function normalizeFilePath(file: string): string {
	return path.normalize(file);
}

function withTrailingSeparator(file: string): string {
	const normalized = normalizeFilePath(file);
	return normalized.endsWith(path.sep) ? normalized : `${normalized}${path.sep}`;
}

function displayPath(file: string): string {
	return path.relative(repoRoot, file);
}

function lineNumberAt(content: string, index: number): number {
	return content.slice(0, index).split('\n').length;
}
