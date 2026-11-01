"""No `grep -q` reads from a pipe, in any tracked shell script or workflow.

-q exits at the first match, and under pipefail the writer's SIGPIPE then fails a pipeline that
matched: a race a slow writer loses (musl's readelf did, in a C smoke). A piped grep sends its
output to /dev/null instead and reads to the end."""
from pathlib import Path
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[3]
SUFFIXES = ('.sh', '.bash', '.yml', '.yaml')
# A pipe (not ||), then grep with -q in a flag cluster (-q, -qx, -Eq, ...), before the next pipe or list.
PIPED_QUIET = re.compile(r'(?<!\|)\|(?!\|)\s*grep\s(?:[^|;&\n]*\s)?-[A-Za-z]*q')


def piped_quiet_greps(path, text):
    """Each `grep -q` reading from a pipe, as path:line, with continuations joined."""
    found = []
    lines = text.split('\n')
    i = 0
    while i < len(lines):
        start, logical = i, lines[i]
        while (logical.rstrip().endswith('\\') or logical.rstrip().endswith('|')) and i + 1 < len(lines):
            i += 1
            logical = logical.rstrip().rstrip('\\') + ' ' + lines[i].strip()
        if PIPED_QUIET.search(logical):
            found.append(f'{path}:{start + 1}')
        i += 1
    return found


def tracked_scripts():
    out = subprocess.run(['git', '-c', 'core.quotepath=off', 'ls-files'], cwd=ROOT, check=True,
                         capture_output=True, text=True).stdout
    return [p for p in out.splitlines() if p.endswith(SUFFIXES) and (ROOT / p).is_file()]


class ShellPipes(unittest.TestCase):
    def test_no_quiet_grep_reads_from_a_pipe(self):
        paths = tracked_scripts()
        self.assertGreater(len(paths), 10, 'git ls-files found too few scripts to trust')
        bad = [hit for path in paths
               for hit in piped_quiet_greps(path, (ROOT / path).read_text(encoding='utf-8'))]
        self.assertEqual(bad, [], 'pipe into grep >/dev/null instead:\n' + '\n'.join(bad))

    def test_the_rule_catches_its_own_fault(self):
        for line in ('readelf -d x | grep -q NEEDED', 'a | grep -qx "$w" || fail', 'a | grep -Eq "x"',
                     'a | tr -d x | grep -E -q y', 'a \\\n  | grep -q y', 'a |\n  grep -qF y', 'a || b | grep -q y'):
            self.assertEqual(piped_quiet_greps('t', line), ['t:1'], line)
        for line in ('grep -q x file', 'a | grep x >/dev/null', 'a | grep -x "$w" > /dev/null',
                     'if grep -qF x "$f"; then a | b; fi', 'grep -q a f || grep -q b f', 'a | grep -v q-thing >/dev/null'):
            self.assertEqual(piped_quiet_greps('t', line), [], line)


if __name__ == '__main__':
    unittest.main()
