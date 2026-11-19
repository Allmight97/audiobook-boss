export interface Span {
	start: number;
	end: number;
	line: number;
	text: string;
	doc: boolean;
}

function pushSpan(source: string, spans: Span[], start: number, end: number, doc: boolean): void {
	let line = 1;
	for (let index = 0; index < start; index += 1) if (source.charCodeAt(index) === 10) line += 1;
	spans.push({ start, end, line, text: source.slice(start, end), doc });
}

type Language = 'rust' | 'css' | 'json' | 'script' | 'jsx';

const EXPRESSION_KEYWORDS = new Set([
	'return',
	'typeof',
	'instanceof',
	'in',
	'of',
	'new',
	'delete',
	'void',
	'throw',
	'case',
	'do',
	'else',
	'yield',
	'await',
]);

const JSX_HEAD = /^<(?:(?=>)|[A-Za-z_$][\w$.:-]*(?=[>/]|\s+(?!extends\b)[^\s,]))/;
const JSX_COMMENT = /\{[ \t]*\/\*[\s\S]*?\*\/[ \t]*\}/y;

function skipQuoted(source: string, index: number, quote: string): number {
	let cursor = index + 1;
	while (cursor < source.length) {
		if (source[cursor] === '\\') {
			cursor += 2;
			continue;
		}
		if (source[cursor] === quote) return cursor + 1;
		cursor += 1;
	}
	return source.length;
}

function skipBlock(source: string, index: number, nested: boolean): number {
	let cursor = index + 2;
	let depth = 1;
	while (cursor < source.length) {
		if (nested && source.startsWith('/*', cursor)) {
			depth += 1;
			cursor += 2;
			continue;
		}
		if (source.startsWith('*/', cursor)) {
			depth -= 1;
			cursor += 2;
			if (depth === 0) return cursor;
			continue;
		}
		cursor += 1;
	}
	return source.length;
}

function regexEnd(source: string, start: number): number | null {
	let cursor = start + 1;
	let inClass = false;
	while (cursor < source.length && source[cursor] !== '\n') {
		const char = source[cursor];
		if (char === '\\') {
			cursor += 2;
			continue;
		}
		if (char === '[') inClass = true;
		else if (char === ']') inClass = false;
		else if (char === '/' && !inClass) {
			cursor += 1;
			while (cursor < source.length && /[a-z]/i.test(source[cursor])) cursor += 1;
			return cursor;
		}
		cursor += 1;
	}
	return null;
}

function skipRustTick(source: string, index: number): number {
	if (source[index + 1] === '\\') {
		let cursor = index + 2;
		while (cursor < source.length && source[cursor] !== "'") {
			cursor += source[cursor] === '\\' ? 2 : 1;
		}
		return Math.min(source.length, cursor + 1);
	}
	if (source[index + 1] !== undefined && source[index + 2] === "'" && source[index + 1] !== "'") {
		return index + 3;
	}
	let cursor = index + 1;
	while (cursor < source.length && /\w/.test(source[cursor])) cursor += 1;
	return cursor;
}

function skipRustRaw(source: string, index: number): number {
	let cursor = index;
	if (source[cursor] === 'b' || source[cursor] === 'c') cursor += 1;
	if (source[cursor] !== 'r') return index;
	cursor += 1;
	let hashes = 0;
	while (source[cursor] === '#') {
		hashes += 1;
		cursor += 1;
	}
	if (source[cursor] !== '"') return index;
	const closer = `"${'#'.repeat(hashes)}`;
	const at = source.indexOf(closer, cursor + 1);
	return at < 0 ? source.length : at + closer.length;
}

function skipRustLiteral(source: string, index: number): number | null {
	if (index > 0 && /[\w$]/.test(source[index - 1])) return null;
	const raw = skipRustRaw(source, index);
	if (raw !== index) return raw;
	const prefixed = source[index] === 'b' || source[index] === 'c';
	if (prefixed && source[index + 1] === '"') return skipQuoted(source, index + 1, '"');
	if (prefixed && source[index + 1] === "'") return skipRustTick(source, index + 1);
	if (source[index] === "'") return skipRustTick(source, index);
	return null;
}

class CScanner {
	readonly spans: Span[] = [];
	private index = 0;
	private expressionStart = true;

	constructor(
		private readonly source: string,
		private readonly language: Language,
	) {}

	run(): Span[] {
		this.scanUntilBrace(false);
		return this.spans;
	}

	private get script(): boolean {
		return this.language === 'script' || this.language === 'jsx';
	}

	private scanUntilBrace(stopAtBrace: boolean): void {
		let depth = 0;
		while (this.index < this.source.length) {
			const char = this.source[this.index];
			if (char === '}' && stopAtBrace && depth === 0) return;
			if (char === '{') depth += 1;
			if (char === '}') depth = Math.max(0, depth - 1);
			this.step();
		}
	}

	private step(): void {
		const { source, index } = this;
		const char = source[index];
		if (/\s/.test(char)) {
			this.index += 1;
			return;
		}
		if (this.comment()) return;
		if (this.literal()) {
			this.expressionStart = false;
			return;
		}
		if (/[\w$]/.test(char)) {
			let end = index;
			while (end < source.length && /[\w$]/.test(source[end])) end += 1;
			this.expressionStart = EXPRESSION_KEYWORDS.has(source.slice(index, end));
			this.index = end;
			return;
		}
		this.expressionStart = !/[)\]]/.test(char);
		this.index += 1;
	}

	private comment(): boolean {
		const { source, index, language } = this;
		if (language !== 'css' && source.startsWith('//', index)) {
			const newline = source.indexOf('\n', index);
			const end = newline < 0 ? source.length : newline;
			const doc = language === 'rust' && /^\/\/[/!]/.test(source.slice(index, index + 3));
			pushSpan(source, this.spans, index, end, doc);
			this.index = end;
			return true;
		}
		if (!source.startsWith('/*', index)) return false;
		const end = skipBlock(source, index, language === 'rust');
		const doc = language === 'rust' && /^\/\*[*!]/.test(source.slice(index, index + 3));
		pushSpan(source, this.spans, index, end, doc);
		this.index = end;
		return true;
	}

	private literal(): boolean {
		const { source, index } = this;
		const char = source[index];
		const quoted = char === '"' || (char === "'" && this.language !== 'rust');
		const end =
			(this.language === 'rust' ? skipRustLiteral(source, index) : null) ??
			(quoted ? skipQuoted(source, index, char) : null);
		if (end !== null) {
			this.index = end;
			return true;
		}
		if (!this.script) return false;
		if (char === '`') {
			this.skipTemplate();
			return true;
		}
		const scriptEnd = this.expressionStart ? this.expressionLiteralEnd(char) : null;
		if (scriptEnd === null) return false;
		this.index = scriptEnd;
		return true;
	}

	private expressionLiteralEnd(char: string): number | null {
		if (char === '/') return regexEnd(this.source, this.index);
		if (char === '<' && this.language === 'jsx') return this.tryJsx();
		return null;
	}

	private skipTemplate(): void {
		this.index += 1;
		while (this.index < this.source.length) {
			const char = this.source[this.index];
			if (char === '\\') {
				this.index += 2;
				continue;
			}
			if (char === '`') {
				this.index += 1;
				return;
			}
			if (char === '$' && this.source[this.index + 1] === '{') {
				this.index += 2;
				this.scanExpression();
				continue;
			}
			this.index += 1;
		}
	}

	private scanExpression(): void {
		this.expressionStart = true;
		this.scanUntilBrace(true);
		this.index += 1;
	}

	private tryJsx(): number | null {
		const saved = { index: this.index, spans: this.spans.length };
		const end = this.jsxElement();
		this.index = saved.index;
		if (end === null) this.spans.length = saved.spans;
		return end;
	}

	private jsxElement(): number | null {
		const head = JSX_HEAD.exec(this.source.slice(this.index, this.index + 200));
		if (!head) return null;
		this.index += head[0].length;
		const opened = this.jsxAttributes();
		if (opened !== 'children') return opened === 'closed' ? this.index : null;
		return this.jsxChildren();
	}

	private jsxAttributes(): 'closed' | 'children' | null {
		const { source } = this;
		while (this.index < source.length) {
			const char = source[this.index];
			if (source.startsWith('/>', this.index)) {
				this.index += 2;
				return 'closed';
			}
			if (char === '>') {
				this.index += 1;
				return 'children';
			}
			if (char === '{') {
				this.index += 1;
				this.scanExpression();
			} else if (char === '"' || char === "'") {
				this.index = skipQuoted(source, this.index, char);
			} else {
				this.index += 1;
			}
		}
		return null;
	}

	private jsxChildren(): number | null {
		const { source } = this;
		while (this.index < source.length) {
			if (source.startsWith('</', this.index)) {
				const close = source.indexOf('>', this.index);
				return close < 0 ? null : close + 1;
			}
			if (source[this.index] === '<') {
				const end = this.jsxElement();
				if (end === null) return null;
				this.index = end;
				continue;
			}
			if (source[this.index] === '{') {
				this.jsxBraces();
				continue;
			}
			this.index += 1;
		}
		return null;
	}

	private jsxBraces(): void {
		JSX_COMMENT.lastIndex = this.index;
		const comment = JSX_COMMENT.exec(this.source);
		if (comment) {
			pushSpan(this.source, this.spans, this.index, this.index + comment[0].length, false);
			this.index += comment[0].length;
			return;
		}
		this.index += 1;
		this.scanExpression();
	}
}

function languageFor(file: string): Language {
	if (file.endsWith('.rs')) return 'rust';
	if (file.endsWith('.css')) return 'css';
	if (file.endsWith('.json')) return 'json';
	return /\.[jt]sx$/.test(file) ? 'jsx' : 'script';
}

export function extractC(file: string, source: string): Span[] {
	return new CScanner(source, languageFor(file)).run();
}

export function extractHtml(source: string): Span[] {
	const spans: Span[] = [];
	let at = source.indexOf('<!--');
	while (at >= 0) {
		const close = source.indexOf('-->', at + 4);
		const end = close < 0 ? source.length : close + 3;
		pushSpan(source, spans, at, end, false);
		at = source.indexOf('<!--', end);
	}
	return spans;
}

type ContextKind = 'double' | 'single' | 'command' | 'arithmetic' | 'backtick' | 'parameter';

interface Context {
	kind: ContextKind;
	depth: number;
}

const HEREDOC = /^<<-?[ \t]*(['"]?)([\w.-]+)\1/;

class ShellLexer {
	private readonly stack: Context[] = [];
	private readonly queued: string[] = [];
	private readonly bodies: string[] = [];

	constructor(
		private readonly source: string,
		private readonly spans: Span[],
	) {}

	line(line: string, offset: number): void {
		if (this.bodies.length > 0) {
			if (line.trim() === this.bodies[0]) this.bodies.shift();
			return;
		}
		let index = 0;
		while (index < line.length) index = this.step(line, index, offset);
		this.bodies.push(...this.queued.splice(0));
	}

	private push(kind: ContextKind, index: number, width: number): number {
		this.stack.push({ kind, depth: 0 });
		return index + width;
	}

	private pop(index: number, width: number): number {
		this.stack.pop();
		return index + width;
	}

	private step(line: string, index: number, offset: number): number {
		const top = this.stack.at(-1);
		if (top === undefined || top.kind === 'command') return this.code(line, index, offset, top);
		const char = line[index];
		if (top.kind === 'single') return char === "'" ? this.pop(index, 1) : index + 1;
		if (char === '\\' && top.kind !== 'arithmetic') return index + 2;
		if (top.kind === 'backtick') return char === '`' ? this.pop(index, 1) : index + 1;
		if (top.kind === 'arithmetic') return this.arithmetic(line, index, top);
		if (top.kind === 'parameter' && char === '}') return this.pop(index, 1);
		if (top.kind === 'double' && char === '"') return this.pop(index, 1);
		return this.expansion(line, index) ?? index + 1;
	}

	private arithmetic(line: string, index: number, top: Context): number {
		const char = line[index];
		if (char === '(') top.depth += 1;
		if (char !== ')') return index + 1;
		if (top.depth > 0) {
			top.depth -= 1;
			return index + 1;
		}
		return line[index + 1] === ')' ? this.pop(index, 2) : index + 1;
	}

	private expansion(line: string, index: number): number | null {
		if (line.startsWith('$((', index)) return this.push('arithmetic', index, 3);
		if (line.startsWith('$(', index)) return this.push('command', index, 2);
		if (line.startsWith('${', index)) return this.push('parameter', index, 2);
		if (line[index] === '`') return this.push('backtick', index, 1);
		return null;
	}

	private code(line: string, index: number, offset: number, top: Context | undefined): number {
		const char = line[index];
		if (char === '\\') return index + 2;
		if (char === '#' && (index === 0 || /\s/.test(line[index - 1]))) {
			pushSpan(this.source, this.spans, offset + index, offset + line.length, false);
			return line.length;
		}
		if (char === "'") return this.push('single', index, 1);
		if (char === '"') return this.push('double', index, 1);
		if (line.startsWith('((', index)) return this.push('arithmetic', index, 2);
		const expanded = this.expansion(line, index);
		if (expanded !== null) return expanded;
		if (top !== undefined && char === '(') top.depth += 1;
		if (top !== undefined && char === ')') {
			if (top.depth === 0) return this.pop(index, 1);
			top.depth -= 1;
		}
		if (line.startsWith('<<<', index)) return index + 3;
		const heredoc = line.startsWith('<<', index) ? HEREDOC.exec(line.slice(index)) : null;
		if (heredoc) {
			this.queued.push(heredoc[2]);
			return index + heredoc[0].length;
		}
		return index + 1;
	}
}

function eachLine(
	source: string,
	visit: (line: string, offset: number, index: number) => void,
): void {
	let offset = 0;
	source.split('\n').forEach((line, index) => {
		visit(line, offset, index);
		offset += line.length + 1;
	});
}

export function extractShell(source: string): Span[] {
	const spans: Span[] = [];
	const lexer = new ShellLexer(source, spans);
	eachLine(source, (line, offset) => {
		if (!(offset === 0 && line.startsWith('#!'))) lexer.line(line, offset);
	});
	return spans;
}

function hashIndex(line: string, toml: boolean): number | null {
	let quote: string | null = null;
	for (let index = 0; index < line.length; index += 1) {
		const char = line[index];
		if (quote) {
			if (char === '\\' && quote !== "'") index += 1;
			else if (char === quote) quote = null;
			continue;
		}
		if (char === '"' || char === "'") {
			quote = char;
			continue;
		}
		if (char !== '#') continue;
		if (toml || index === 0 || /\s/.test(line[index - 1])) return index;
	}
	return null;
}

const BLOCK_SCALAR = /^(\s*)(-\s+)?(?:([\w.-]+)\s*:\s*)?[|>][-+\d]*\s*(?:#.*)?$/;

interface YamlLine {
	mode: 'yaml' | 'literal' | 'run';
	block: number;
}

function yamlLines(lines: string[]): YamlLine[] {
	const kinds: YamlLine[] = lines.map(() => ({ mode: 'yaml', block: -1 }));
	for (let index = 0; index < lines.length; index += 1) {
		const open = BLOCK_SCALAR.exec(lines[index]);
		if (!open || (open[3] === undefined && open[2] === undefined)) continue;
		const indent = open[1].length + (open[3] === undefined ? 0 : (open[2]?.length ?? 0));
		const mode = open[3] === 'run' ? 'run' : 'literal';
		let child = index + 1;
		for (; child < lines.length; child += 1) {
			if (lines[child].trim() === '') continue;
			if ((/^\s*/.exec(lines[child])?.[0].length ?? 0) <= indent) break;
			kinds[child] = { mode, block: index };
		}
		index = child - 1;
	}
	return kinds;
}

export function extractYaml(source: string): Span[] {
	const lines = source.split('\n');
	const kinds = yamlLines(lines);
	const spans: Span[] = [];
	let lexer: ShellLexer | null = null;
	let block = -1;
	eachLine(source, (line, offset, index) => {
		const { mode, block: owner } = kinds[index];
		if (mode === 'run') {
			if (lexer === null || owner !== block) lexer = new ShellLexer(source, spans);
			block = owner;
			lexer.line(line, offset);
			return;
		}
		const hash = mode === 'yaml' ? hashIndex(line, false) : null;
		if (hash !== null) pushSpan(source, spans, offset + hash, offset + line.length, false);
	});
	return spans;
}

export function extractToml(source: string): Span[] {
	const spans: Span[] = [];
	let inside = false;
	eachLine(source, (line, offset) => {
		const marks = line.match(/"""|'''/g)?.length ?? 0;
		const continued = inside;
		inside = marks % 2 === 1 ? !inside : inside;
		if (continued) return;
		const hash = hashIndex(line, true);
		if (hash !== null) pushSpan(source, spans, offset + hash, offset + line.length, false);
	});
	return spans;
}
