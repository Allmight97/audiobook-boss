#!/usr/bin/env python3
"""
Analyzes line composition of Rust modules over 400 lines.

Categorizes lines as:
- Comment lines: Lines starting with //, /*, */, or within block comments
- Doc comment lines: Lines starting with /// or //!
- Test code lines: Lines within #[cfg(test)] modules, #[test] functions, or test-related helper code
- Blank lines: Empty lines or lines with only whitespace
- Implementation code lines: All remaining lines that contain actual logic
"""

import os
import re
from typing import Dict, List, Tuple, Optional

class LineAnalyzer:
    def __init__(self, content: str):
        self.lines = content.split('\n')
        self.total_lines = len(self.lines)
        self.in_block_comment = False
        self.in_test_block = False
        self.test_block_depth = 0
        
        # Counters
        self.comment_lines = 0
        self.doc_comment_lines = 0
        self.test_code_lines = 0
        self.blank_lines = 0
        self.implementation_lines = 0
        
    def analyze(self) -> Dict[str, int]:
        """Analyze all lines and return counts."""
        for line_num, line in enumerate(self.lines, 1):
            self._analyze_line(line.rstrip())
            
        return {
            'total_lines': self.total_lines,
            'comment_lines': self.comment_lines,
            'doc_comment_lines': self.doc_comment_lines,
            'test_code_lines': self.test_code_lines,
            'blank_lines': self.blank_lines,
            'implementation_lines': self.implementation_lines
        }
    
    def _analyze_line(self, line: str) -> None:
        """Analyze a single line and categorize it."""
        # Handle empty or whitespace-only lines
        if not line or line.isspace():
            self.blank_lines += 1
            return
            
        stripped = line.strip()
        
        # Handle block comment transitions
        if '/*' in line and '*/' in line and line.find('/*') < line.find('*/'):
            # Single line block comment
            if self._is_doc_comment_line(stripped):
                self.doc_comment_lines += 1
            else:
                self.comment_lines += 1
            return
        elif '/*' in line:
            self.in_block_comment = True
            if self._is_doc_comment_line(stripped):
                self.doc_comment_lines += 1
            else:
                self.comment_lines += 1
            return
        elif '*/' in line:
            self.in_block_comment = False
            if self._is_doc_comment_line(stripped):
                self.doc_comment_lines += 1
            else:
                self.comment_lines += 1
            return
        elif self.in_block_comment:
            if self._is_doc_comment_line(stripped):
                self.doc_comment_lines += 1
            else:
                self.comment_lines += 1
            return
            
        # Handle test block detection
        self._update_test_block_status(line)
        
        # Categorize the line
        if self._is_doc_comment_line(stripped):
            self.doc_comment_lines += 1
        elif self._is_regular_comment_line(stripped):
            self.comment_lines += 1
        elif self.in_test_block or self._is_test_related_line(stripped):
            self.test_code_lines += 1
        else:
            self.implementation_lines += 1
    
    def _is_doc_comment_line(self, line: str) -> bool:
        """Check if line is a documentation comment."""
        return line.startswith('///') or line.startswith('//!')
    
    def _is_regular_comment_line(self, line: str) -> bool:
        """Check if line is a regular comment (not doc comment)."""
        return line.startswith('//') and not self._is_doc_comment_line(line)
    
    def _is_test_related_line(self, line: str) -> bool:
        """Check if line is test-related (outside of test blocks)."""
        test_patterns = [
            r'#\[test\]',
            r'#\[cfg\(test\)\]',
            r'assert!',
            r'assert_eq!',
            r'assert_ne!',
            r'expect\(',
            r'mock',  # Mock-related code
        ]
        
        for pattern in test_patterns:
            if re.search(pattern, line):
                return True
        return False
    
    def _update_test_block_status(self, line: str) -> None:
        """Update test block tracking based on the line."""
        stripped = line.strip()
        
        # Check for test module or function start
        if re.search(r'#\[cfg\(test\)\]', line):
            # Next non-comment, non-blank line should start a test block
            return
        
        if (re.search(r'#\[test\]', line) or 
            re.search(r'mod.*tests\s*{', line) or
            (self.test_block_depth == 0 and re.search(r'#\[cfg\(test\)\]', line))):
            self.in_test_block = True
            self.test_block_depth = 1
        
        if self.in_test_block:
            # Count braces to track nesting
            open_braces = stripped.count('{')
            close_braces = stripped.count('}')
            self.test_block_depth += open_braces - close_braces
            
            if self.test_block_depth <= 0:
                self.in_test_block = False
                self.test_block_depth = 0

def analyze_file(file_path: str) -> Dict[str, int]:
    """Analyze a single Rust file."""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    analyzer = LineAnalyzer(content)
    return analyzer.analyze()

def format_results(file_path: str, results: Dict[str, int]) -> str:
    """Format analysis results for display."""
    total = results['total_lines']
    
    output = [f"\n## {os.path.basename(file_path)} ({total} lines)"]
    output.append("-" * 60)
    
    # Raw counts
    output.append(f"**Comment lines**: {results['comment_lines']} lines")
    output.append(f"**Doc comment lines**: {results['doc_comment_lines']} lines")
    output.append(f"**Test code lines**: {results['test_code_lines']} lines")
    output.append(f"**Blank lines**: {results['blank_lines']} lines")
    output.append(f"**Implementation code lines**: {results['implementation_lines']} lines")
    output.append(f"**Total**: {total} lines")
    
    # Percentages
    output.append("\n**Percentage breakdown**:")
    output.append(f"- Comment lines: {results['comment_lines']/total*100:.1f}%")
    output.append(f"- Doc comment lines: {results['doc_comment_lines']/total*100:.1f}%")
    output.append(f"- Test code lines: {results['test_code_lines']/total*100:.1f}%")
    output.append(f"- Blank lines: {results['blank_lines']/total*100:.1f}%")
    output.append(f"- Implementation code lines: {results['implementation_lines']/total*100:.1f}%")
    
    return "\n".join(output)

def main():
    """Main analysis function."""
    # The 8 modules over 400 lines
    modules = [
        "/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/media_pipeline.rs",  # 736 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/processor.rs",       # 644 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/cleanup.rs",        # 503 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/progress.rs",       # 497 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/tests/settings_validation_integration.rs", # 482 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/tests/unit/audio/processor_tests.rs",      # 412 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/src/tests_integration.rs",    # 410 lines
        "/Users/jstar/Projects/audiobook-boss/src-tauri/src/audio/file_list.rs",     # 400 lines
    ]
    
    print("# Line Composition Analysis for Rust Modules Over 400 Lines")
    print("=" * 80)
    
    all_results = {}
    
    for module_path in modules:
        if os.path.exists(module_path):
            try:
                results = analyze_file(module_path)
                all_results[module_path] = results
                print(format_results(module_path, results))
            except Exception as e:
                print(f"\nError analyzing {module_path}: {e}")
        else:
            print(f"\nFile not found: {module_path}")
    
    # Summary
    if all_results:
        print("\n" + "=" * 80)
        print("# SUMMARY")
        print("=" * 80)
        
        total_lines = sum(r['total_lines'] for r in all_results.values())
        total_comment = sum(r['comment_lines'] for r in all_results.values())
        total_doc_comment = sum(r['doc_comment_lines'] for r in all_results.values())
        total_test = sum(r['test_code_lines'] for r in all_results.values())
        total_blank = sum(r['blank_lines'] for r in all_results.values())
        total_implementation = sum(r['implementation_lines'] for r in all_results.values())
        
        print(f"**Total lines across all modules**: {total_lines}")
        print(f"**Comment lines**: {total_comment} ({total_comment/total_lines*100:.1f}%)")
        print(f"**Doc comment lines**: {total_doc_comment} ({total_doc_comment/total_lines*100:.1f}%)")
        print(f"**Test code lines**: {total_test} ({total_test/total_lines*100:.1f}%)")
        print(f"**Blank lines**: {total_blank} ({total_blank/total_lines*100:.1f}%)")
        print(f"**Implementation lines**: {total_implementation} ({total_implementation/total_lines*100:.1f}%)")
        
        print(f"\n**Modules analyzed**: {len(all_results)}")
        for path in all_results.keys():
            filename = os.path.basename(path)
            lines = all_results[path]['total_lines']
            print(f"- {filename}: {lines} lines")

if __name__ == "__main__":
    main()
