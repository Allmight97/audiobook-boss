import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';

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
	const sourceFile = ts.createSourceFile(file, block.content, ts.ScriptTarget.Latest, true);
	for (const statement of sourceFile.statements) {
		if (ts.isImportDeclaration(statement)) {
			checkImportDeclaration(file, fullContent, block, sourceFile, statement);
		} else if (ts.isExportDeclaration(statement)) {
			checkExportDeclaration(file, fullContent, block, sourceFile, statement);
		}
	}

	if (!allowsRawTauriCore(file)) {
		checkRawTauriInvokeUsage(file, fullContent, block, sourceFile);
	}
}

function checkImportDeclaration(
	file: string,
	fullContent: string,
	block: SourceBlock,
	sourceFile: ts.SourceFile,
	statement: ts.ImportDeclaration,
): void {
	const source = moduleSpecifierText(statement.moduleSpecifier);
	const importClause = statement.importClause;
	if (!source || !importClause || importClause.isTypeOnly) {
		return;
	}

	const line = lineNumberAt(fullContent, block.start + statement.getStart(sourceFile));
	if (source === '@tauri-apps/api/core' && !allowsRawTauriCore(file)) {
		pushRawTauriCoreImportViolations(file, line, importClause);
		return;
	}

	if (!isGeneratedTauriImport(file, source)) {
		return;
	}

	const namedImports = importNamedValues(importClause);
	const hasValueNamespaceImport =
		importClause.namedBindings !== undefined && ts.isNamespaceImport(importClause.namedBindings);
	pushNamedValueViolations(file, line, namedImports);

	if (hasValueNamespaceImport) {
		violations.push({
			file: displayPath(file),
			line,
			message:
				'generated Tauri namespace value imports are not allowed; import commands or events through the owning boundary file',
		});
	}
}

function checkExportDeclaration(
	file: string,
	fullContent: string,
	block: SourceBlock,
	sourceFile: ts.SourceFile,
	statement: ts.ExportDeclaration,
): void {
	const source = moduleSpecifierText(statement.moduleSpecifier);
	if (!source || statement.isTypeOnly || !isGeneratedTauriImport(file, source)) {
		return;
	}

	const line = lineNumberAt(fullContent, block.start + statement.getStart(sourceFile));
	const exportClause = statement.exportClause;
	if (!exportClause || ts.isNamespaceExport(exportClause)) {
		violations.push({
			file: displayPath(file),
			line,
			message:
				'generated Tauri value re-exports are not allowed; import commands or events through the owning boundary file',
		});
		return;
	}

	pushNamedValueViolations(file, line, namedExports(exportClause));
}

function checkRawTauriInvokeUsage(
	file: string,
	fullContent: string,
	block: SourceBlock,
	sourceFile: ts.SourceFile,
): void {
	function visit(node: ts.Node): void {
		if (ts.isIdentifier(node) && node.text === '__TAURI_INVOKE') {
			violations.push({
				file: displayPath(file),
				line: lineNumberAt(fullContent, block.start + node.getStart(sourceFile)),
				message: 'raw __TAURI_INVOKE usage must stay out of runtime app code; use tauriClient',
			});
		}
		ts.forEachChild(node, visit);
	}

	visit(sourceFile);
}

function pushRawTauriCoreImportViolations(
	file: string,
	line: number,
	importClause: ts.ImportClause,
): void {
	if (importClause.name) {
		violations.push({
			file: displayPath(file),
			line,
			message: 'raw Tauri core default imports must stay out of runtime app code; use tauriClient',
		});
	}

	const namedBindings = importClause.namedBindings;
	if (!namedBindings) {
		return;
	}

	if (ts.isNamespaceImport(namedBindings)) {
		violations.push({
			file: displayPath(file),
			line,
			message:
				'raw Tauri core namespace imports must stay out of runtime app code; use tauriClient',
		});
		return;
	}

	for (const specifier of namedBindings.elements) {
		if (specifier.isTypeOnly) {
			continue;
		}
		const name = specifier.propertyName?.text ?? specifier.name.text;
		if (name === 'invoke') {
			violations.push({
				file: displayPath(file),
				line,
				message: "raw Tauri 'invoke' imports must stay out of runtime app code; use tauriClient",
			});
		}
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

function moduleSpecifierText(moduleSpecifier: ts.Expression | undefined): string | null {
	if (!moduleSpecifier || !ts.isStringLiteral(moduleSpecifier)) {
		return null;
	}
	return moduleSpecifier.text;
}

function importNamedValues(importClause: ts.ImportClause): Set<string> {
	const namedBindings = importClause.namedBindings;
	if (!namedBindings || !ts.isNamedImports(namedBindings)) {
		return new Set();
	}

	const names = new Set<string>();
	for (const specifier of namedBindings.elements) {
		if (specifier.isTypeOnly) {
			continue;
		}
		names.add(specifier.propertyName?.text ?? specifier.name.text);
	}
	return names;
}

function namedExports(exportClause: ts.NamedExports): Set<string> {
	const names = new Set<string>();
	for (const specifier of exportClause.elements) {
		if (specifier.isTypeOnly) {
			continue;
		}
		names.add(specifier.propertyName?.text ?? specifier.name.text);
	}
	return names;
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
