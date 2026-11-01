#!/usr/bin/env python3
"""Classify a pull request's files into the classes ci.yml's gates read.

    gh api "repos/<owner>/<repo>/pulls/<n>/files" --paginate \\
        --jq '.[] | [.filename, (.previous_filename // "")] | @tsv' | python3 pr_changes.py
    python3 pr_changes.py --all        # off a pull request: every class true, docs_only false

Reads `filename<TAB>previous_filename` lines (the second field empty when the file was not
renamed) and prints one `key=true|false` line per class, in ci.yml's output order, for
`tee -a "$GITHUB_OUTPUT"`. The rules are the design spec's table
(docs/superpowers/specs/2026-10-31-pr-board-design.md, "Change detection"): a path belongs to
the first class that names it, `ci` first, since a workflow edit can reach any job; a rename
belongs to every class either of its paths matches. `ci` and `core` imply every binding; a
path no rule names, an empty list, or a list at the API's 3,000-file cap is `everything`,
the fail-safe: the whole board runs rather than guessing. Standard library only, so the job
runs it with no install.
"""
import sys

KEYS = ['core', 'ci', 'docs_only', 'csharp', 'java', 'python', 'ruby', 'php', 'swift', 'typescript', 'r', 'go']
BINDINGS = KEYS[3:]
API_CAP = 3000

# (class, rules), first match wins. A rule is `<dir>/**` (every path under the directory),
# `**/*.<ext>` (that suffix at any depth, the root included) or an exact path.
RULES = [
    ('ci', ['.github/**']),
    ('docs', ['**/*.md', 'docs/**', 'LICENSE',
              '.editorconfig', '.gitattributes', '.gitignore', '.claude/**']),
    ('core', ['rust/**', 'include/**', 'c/**', 'corpus/**']),
    ('csharp', ['csharp/**', 'global.json']),
    ('java', ['java/**']),
    ('python', ['python/**']),
    ('ruby', ['ruby/**']),
    ('php', ['php/**', 'composer.json']),
    ('swift', ['swift/**', 'Package.swift']),
    ('typescript', ['typescript/**']),
    ('r', ['r/**']),
    ('go', ['go/**']),
]


def matches(rule, path):
    if rule.endswith('/**'):
        return path.startswith(rule[:-2])
    if rule.startswith('**/*.'):
        return path.endswith(rule[4:])
    return path == rule


def class_of(path):
    for name, rules in RULES:
        if any(matches(rule, path) for rule in rules):
            return name
    return 'everything'


def everything():
    return {**{key: True for key in KEYS}, 'docs_only': False}


def classify(paths):
    """`paths`: (filename, previous_filename or None) pairs, as the API lists them."""
    if not paths or len(paths) >= API_CAP:
        return everything()
    classes = {class_of(path) for pair in paths for path in pair if path}
    if 'everything' in classes:
        return everything()
    result = {key: False for key in KEYS}
    result['ci'] = 'ci' in classes
    result['core'] = 'core' in classes or result['ci']
    for binding in BINDINGS:
        result[binding] = binding in classes or result['core']
    result['docs_only'] = classes <= {'docs'}
    return result


def main(argv):
    if argv == ['--all']:
        result = everything()
    elif argv:
        print(f'pr_changes: usage: pr_changes.py [--all] < files.tsv, not {argv}', file=sys.stderr)
        return 2
    else:
        paths = []
        for number, line in enumerate(sys.stdin, 1):
            line = line.rstrip('\n')
            if not line:
                continue
            fields = line.split('\t')
            if len(fields) != 2 or not fields[0]:
                print(f'pr_changes: line {number} is not filename<TAB>previous_filename: {line!r}', file=sys.stderr)
                return 2
            paths.append((fields[0], fields[1] or None))
        result = classify(paths)
    for key in KEYS:
        print(f'{key}={"true" if result[key] else "false"}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
