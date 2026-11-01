#!/usr/bin/env python3
"""The PR board's verdict, ci.yml's `pr-board` job: the one status to read.

Reads NEEDS (`toJSON(needs)`), CHANGES (`toJSON(needs.changes.outputs)`), GITHUB_EVENT_NAME,
GITHUB_REF and PURPOSE (`inputs.purpose`, empty off a dispatch), and exits 1 when any needed
job failed or was cancelled, or was skipped while its gate said it should run. Master is not
branch-protected and a skipped job satisfies a required check, so without this a too-narrow
classifier (.github/scripts/pr_changes.py) would be invisible. The gates below mirror
ci.yml's `if:` lines one for one; test_pr_board.py holds them to the file. A job this script
does not know is gated on: a new job that skips is red until a gate is written for it.
"""
import json
import os
import sys

BINDINGS = ['csharp', 'java', 'python', 'ruby', 'php', 'swift', 'typescript', 'r', 'go']
# Every job but the two aggregates, in the order a failure is named: a red lint before the
# suites it skipped, the core before what waits on it.
# The release tail, in dependency order: the Forge resolver, the eight pack jobs, the
# inventory, CodeQL and B. Run on a master dispatch with purpose=release only
# (prepare-release.yml's), never on a pull request, the weekly schedule or a purpose=board
# dispatch, whatever changed.
TAIL = ['forge', 'nuget-org-pack', 'crate-pack', 'maven-pack', 'ruby-pack', 'npm-pack', 'r-pack', 'c-pack', 'pypi-pack',
        'release-prepare', 'codeql', 'stage-release']
JOBS = (['changes'] + [f'lint-{b}' for b in BINDINGS] + ['lint-go-full', 'core', 'core-checks', 'build-static', 'build-android']
        + [f'test-{b}' for b in BINDINGS]
        + ['pack-c', 'build-wasm', 'build-wheels', 'test-nuget-package', 'test-npm-package', 'test-apple-mobile', 'test-android']
        + TAIL)


def gates(outputs, event, ref='', purpose=''):
    """Expected-to-run, per job, from `changes`' outputs (strings), the event name, the ref and the
    dispatch purpose."""
    on = {key: value == 'true' for key, value in outputs.items()}
    core_or_ci = on.get('core', False) or on.get('ci', False)

    def binding(name):
        return on.get(name, False) or core_or_ci

    release = event != 'pull_request'
    result = {'changes': True, 'core': not on.get('docs_only', False),
              'build-static': core_or_ci or on.get('csharp', False) or on.get('swift', False),
              'build-android': core_or_ci, 'core-checks': core_or_ci, 'pack-c': core_or_ci,
              'build-wasm': core_or_ci or on.get('csharp', False) or on.get('typescript', False) or on.get('go', False),
              'build-wheels': binding('python'), 'test-nuget-package': binding('csharp'),
              'test-npm-package': binding('typescript'), 'test-apple-mobile': release, 'test-android': release,
              'lint-go-full': binding('go')}
    for name in BINDINGS:
        result[f'lint-{name}'] = result[f'test-{name}'] = binding(name)
    train = event == 'workflow_dispatch' and ref == 'refs/heads/master' and purpose == 'release'
    for name in TAIL:
        result[name] = train
    return result


def verdict(results, gates_):
    """(green, why): first any failed or cancelled job, in JOBS order, then any skipped job
    whose gate is on. Jobs the gates do not name are gated on."""
    order = [name for name in JOBS if name in results] + sorted(name for name in results if name not in JOBS)
    for name in order:
        if results[name] in ('failure', 'cancelled'):
            return False, f'{name} failed'
    for name in order:
        if results[name] == 'skipped' and gates_.get(name, True):
            return False, f'{name} skipped but gated on'
    return True, 'ok'


def main():
    needs = json.loads(os.environ['NEEDS'])
    outputs = json.loads(os.environ['CHANGES'] or '{}') or {}
    event = os.environ.get('GITHUB_EVENT_NAME', '')
    results = {name: job.get('result', '') for name, job in needs.items()}
    gates_ = gates(outputs, event, os.environ.get('GITHUB_REF', ''), os.environ.get('PURPOSE', ''))
    for name in sorted(results, key=lambda n: JOBS.index(n) if n in JOBS else len(JOBS)):
        gate = 'on ' if gates_.get(name, True) else 'off'
        print(f'{results[name]:9} gate {gate}  {name}')
    green, why = verdict(results, gates_)
    print(('ok' if green else '::error::pr-board: ') + ('' if green else why))
    return 0 if green else 1


if __name__ == '__main__':
    sys.exit(main())
