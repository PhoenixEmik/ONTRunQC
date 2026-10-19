#!/usr/bin/env python3
"""Check repository-local Markdown targets/anchors without network dependencies.

External URLs are deliberately not fetched. Code fences are excluded. This checks
our inline links/images and explicit HTML IDs, not every possible Markdown form.
"""
from collections import Counter
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]


def prose(text):
    fence = None
    lines = []
    for line in text.splitlines():
        marker = re.match(r'^\s*(`{3,}|~{3,})', line)
        if marker:
            token = marker[1]
            if fence is None:
                fence = (token[0], len(token))
            elif token[0] == fence[0] and len(token) >= fence[1]:
                fence = None
            continue
        if fence is None:
            lines.append(line)
    return '\n'.join(lines)


def slug(title):
    title = re.sub(r'\[([^]]+)\]\([^)]*\)', r'\1', title)
    title = re.sub(r'<[^>]+>', '', title).lower().replace('`', '')
    return re.sub(r'[^\w\-\s]', '', title).replace(' ', '-')


def anchors(text):
    text = prose(text)
    result = set(re.findall(r'\bid=["\']([^"\']+)["\']', text))
    counts = Counter()
    for title in re.findall(r'^#{1,6}\s+(.+?)\s*#*\s*$', text, re.M):
        base = slug(title)
        number = counts[base]
        result.add(base + (f'-{number}' if number else ''))
        counts[base] += 1
    return result


def check(paths):
    failures = []
    checked = 0
    cache = {}
    for path in paths:
        label = path.relative_to(ROOT) if path.is_relative_to(ROOT) else path
        content = prose(path.read_text())
        for target in re.findall(r'!?\[[^]\n]*\]\(([^)\n]+)\)', content):
            target = target.strip()
            if target.startswith('<'):
                target = target[1:target.index('>')]
            else:
                target = target.split(' "', 1)[0]
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc:
                continue
            checked += 1
            destination = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path.resolve()
            if not destination.exists():
                failures.append(f'{label}: missing target {target}')
            elif parsed.fragment and destination.suffix == '.md':
                if destination not in cache:
                    cache[destination] = anchors(destination.read_text())
                if unquote(parsed.fragment) not in cache[destination]:
                    failures.append(f'{label}: missing anchor {target}')
    return checked, failures


def main():
    names = subprocess.check_output(
        ['git', 'ls-files', '--cached', '--others', '--exclude-standard', '*.md'],
        cwd=ROOT, text=True).splitlines()
    paths = [ROOT / name for name in sorted(set(names))]
    checked, failures = check(paths)
    for failure in failures:
        print(failure, file=sys.stderr)
    print(f'{len(paths)} Markdown files, {checked} local links checked; {len(failures)} failures.')
    return bool(failures)


if __name__ == '__main__':
    sys.exit(main())
