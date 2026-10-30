"""Every tracked text file is UTF-8 without a BOM, with LF line endings and no CR anywhere.

.gitattributes (`* text=auto eol=lf`) holds git checkouts to LF and .editorconfig asks editors
for UTF-8 and LF; this is what fails when either is bypassed, by a template that writes a BOM
or a file committed from a tool that ignores both. Text and binary are git's own call, read
from `git ls-files --eol` over the working tree, so a committed library is never inspected."""
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[3]


def problems(path, data):
    """What is wrong with one text file's bytes, as short phrases (empty when nothing is)."""
    found = []
    if data.startswith(b'\xef\xbb\xbf'):
        found.append('a UTF-8 BOM')
    try:
        data.decode('utf-8')
    except UnicodeDecodeError:
        found.append('bytes that are not UTF-8')
    if b'\r' in data:
        found.append('a CR (line endings must be LF)')
    return [f'{path}: {p}' for p in found]


def tracked_text_files():
    """Tracked paths whose working-tree content git classifies as text."""
    out = subprocess.run(['git', '-c', 'core.quotepath=off', 'ls-files', '--eol'], cwd=ROOT,
                         check=True, capture_output=True, text=True).stdout
    paths = []
    for line in out.splitlines():
        info, path = line.split('\t', 1)
        worktree = info.split()[1]  # w/lf, w/crlf, w/mixed, w/-text, or w/ (a link, or gone)
        if worktree not in ('w/', 'w/-text'):
            paths.append(path)
    return paths


class TextFiles(unittest.TestCase):
    def test_every_tracked_text_file_is_utf8_lf_without_a_bom(self):
        paths = tracked_text_files()
        self.assertGreater(len(paths), 300, 'git ls-files found too few text files to trust')
        bad = [p for path in paths for p in problems(path, (ROOT / path).read_bytes())]
        self.assertEqual(bad, [], '\n'.join(bad))

    def test_each_rule_catches_its_own_fault(self):
        self.assertEqual(problems('ok', b'plain\nlines\n'), [])
        self.assertEqual(problems('bom', b'\xef\xbb\xbfx\n'), ['bom: a UTF-8 BOM'])
        self.assertEqual(problems('crlf', b'x\r\ny\r\n'), ['crlf: a CR (line endings must be LF)'])
        self.assertEqual(problems('cr', b'x\ry'), ['cr: a CR (line endings must be LF)'])
        self.assertEqual(problems('latin1', b'caf\xe9\n'), ['latin1: bytes that are not UTF-8'])


if __name__ == '__main__':
    unittest.main()
