#!/usr/bin/env python3
import os
import re
from pathlib import Path
from collections import defaultdict

def count_implementation_lines(file_path):
    """Count implementation lines, excluding comments, tests, and blank lines."""
    if not os.path.exists(file_path):
        return 0, 0, 0, 0  # total, comments, blank, implementation
    
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        lines = f.readlines()
    
    total_lines = len(lines)
    comment_lines = 0
    blank_lines = 0
    test_lines = 0
    
    in_block_comment = False
    in_test_block = False
    
    for i, line in enumerate(lines):
        stripped = line.strip()
        
        # Count blank lines
        if not stripped:
            blank_lines += 1
            continue
        
        # Detect file extensions
        is_rust = file_path.endswith('.rs')
        is_typescript = file_path.endswith('.ts') or file_path.endswith('.tsx')
        is_javascript = file_path.endswith('.js') or file_path.endswith('.jsx')
        
        # Handle block comments
        if is_rust:
            if '/*' in stripped and '*/' in stripped and stripped.startswith('//'):
                # Single line comment that also has block comment markers
                comment_lines += 1
                continue
            elif '/*' in stripped:
                in_block_comment = True
                comment_lines += 1
                if '*/' in stripped:
                    in_block_comment = False
                continue
            elif in_block_comment:
                comment_lines += 1
                if '*/' in stripped:
                    in_block_comment = False
                continue
            elif stripped.startswith('//') or stripped.startswith('///'):
                comment_lines += 1
                continue
        
        if is_typescript or is_javascript:
            if '/*' in stripped:
                in_block_comment = True
                comment_lines += 1
                if '*/' in stripped:
                    in_block_comment = False
                continue
            elif in_block_comment:
                comment_lines += 1
                if '*/' in stripped:
                    in_block_comment = False
                continue
            elif stripped.startswith('//'):
                comment_lines += 1
                continue
        
        # Detect test-related code (heuristic)
        is_test_file = 'test' in file_path.lower() or '_test.' in file_path or '.test.' in file_path
        
        # Check for test functions/blocks
        test_patterns = [
            r'^#\[cfg\(test\)\]',  # Rust test cfg
            r'^#\[test\]',         # Rust test attribute
            r'^\s*fn\s+test_',     # Rust test function
            r'^\s*describe\s*\(',  # JS/TS describe block
            r'^\s*it\s*\(',        # JS/TS it block
            r'^\s*test\s*\(',      # JS/TS test block
            r'^\s*expect\s*\(',    # JS/TS expect calls
            r'^\s*assert',         # Assert statements
        ]
        
        for pattern in test_patterns:
            if re.match(pattern, stripped):
                in_test_block = True
                break
        
        # If we're in a test block and this is a closing brace, exit test block
        if in_test_block and stripped == '}':
            in_test_block = False
            test_lines += 1
            continue
        
        if in_test_block or is_test_file:
            test_lines += 1
            continue
    
    implementation_lines = total_lines - comment_lines - blank_lines - test_lines
    return total_lines, comment_lines, blank_lines, test_lines, implementation_lines

def analyze_modules():
    """Analyze all modules and their implementation line counts."""
    
    # Define modules based on directory structure
    modules = []
    
    # TypeScript/JavaScript modules
    src_dir = Path('./src')
    if src_dir.exists():
        for file_path in src_dir.rglob('*.ts'):
            if file_path.is_file():
                modules.append(('TypeScript', str(file_path.relative_to('.'))))
        
        for file_path in src_dir.rglob('*.js'):
            if file_path.is_file():
                modules.append(('JavaScript', str(file_path.relative_to('.'))))
    
    # Rust modules  
    rust_src_dir = Path('./src-tauri/src')
    if rust_src_dir.exists():
        for file_path in rust_src_dir.rglob('*.rs'):
            if file_path.is_file():
                modules.append(('Rust', str(file_path.relative_to('.'))))
    
    # Analyze each module
    results = []
    
    for lang, module_path in modules:
        total, comments, blank, test, impl = count_implementation_lines(module_path)
        
        results.append({
            'language': lang,
            'module': module_path,
            'total_lines': total,
            'comment_lines': comments,
            'blank_lines': blank,
            'test_lines': test,
            'implementation_lines': impl
        })
    
    # Sort by implementation lines descending
    results.sort(key=lambda x: x['implementation_lines'], reverse=True)
    
    print("Module Implementation Line Analysis")
    print("=" * 80)
    print(f"{'Module':<40} {'Total':<6} {'Comments':<8} {'Blank':<6} {'Test':<6} {'Impl':<6} {'Over 400?'}")
    print("-" * 80)
    
    modules_over_400 = []
    
    for result in results:
        module_name = result['module']
        if len(module_name) > 37:
            module_name = "..." + module_name[-34:]
        
        over_400 = "YES" if result['implementation_lines'] > 400 else "NO"
        if result['implementation_lines'] > 400:
            modules_over_400.append(result)
        
        print(f"{module_name:<40} {result['total_lines']:<6} {result['comment_lines']:<8} "
              f"{result['blank_lines']:<6} {result['test_lines']:<6} {result['implementation_lines']:<6} {over_400}")
    
    print("\n" + "=" * 80)
    print(f"SUMMARY: {len(modules_over_400)} modules exceed 400 implementation lines")
    
    if modules_over_400:
        print("\nModules exceeding 400 implementation lines:")
        for result in modules_over_400:
            print(f"  • {result['module']}: {result['implementation_lines']} lines")

if __name__ == "__main__":
    analyze_modules()
