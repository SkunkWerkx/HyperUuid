"""Always dispatch the full CI board on master and check its resolved source SHA."""
import os
import subprocess

from release_guard import api, validate_run


def dispatch(repo, sha):
    # The versioned API returns this dispatch's ID, avoiding a latest-run lookup race.
    run_id = subprocess.check_output([
        'gh', 'api', '--method', 'POST', '-H', 'X-GitHub-Api-Version: 2026-03-10',
        f'repos/{repo}/actions/workflows/ci.yml/dispatches', '-f', 'ref=master',
        '--jq', '.workflow_run_id'], text=True).strip()
    try:
        validate_run(api(repo, f'actions/runs/{run_id}'), sha, run_id, repo, completed=False)
    except ValueError:
        # A branch race must not later produce a successful workflow_run staging event.
        subprocess.run(['gh', 'run', 'cancel', run_id, '--repo', repo], check=True)
        raise
    print(f'CI run {run_id} dispatched on master at preparation source {sha}. '
          'After staging succeeds, inspect and manually tag its staging child.')
    return run_id


if __name__ == '__main__':
    dispatch(os.environ['GITHUB_REPOSITORY'], os.environ['SOURCE_SHA'])
