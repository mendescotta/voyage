#!/usr/bin/env bash
# Exercise real subprocess exit propagation without restarting the test machine.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"
python3 - "$HERE/resources/backend/reboot.sh" <<'PY'
import pathlib, subprocess, sys, tempfile
with tempfile.TemporaryDirectory() as temp:
    root = pathlib.Path(temp)
    source = pathlib.Path(sys.argv[1]).read_text()
    comm = root / 'comm'
    init_exe = root / 'init-exe'
    (root / 'dinit').write_text('')
    init_exe.symlink_to(root / 'dinit')
    source = source.replace('/proc/1/comm', str(comm)).replace('/proc/1/exe', str(init_exe))
    for path in ['/usr/bin/dinit-shutdown', '/usr/bin/reboot', '/usr/sbin/reboot', '/sbin/reboot', '/bin/reboot']:
        source = source.replace(path, str(root / path.strip('/').replace('/', '-')))
    script = root / 'reboot.sh'; script.write_text(source)
    dinit = root / 'usr-bin-dinit-shutdown'
    dinit.write_text('#!/bin/sh\nprintf "%s\\n" "$*"\nexit 17\n'); dinit.chmod(0o755)
    reboot = root / 'usr-bin-reboot'
    reboot.write_text('#!/bin/sh\necho reboot\nexit 18\n'); reboot.chmod(0o755)
    comm.write_text('dinit\n')
    result = subprocess.run(['bash', str(script)], capture_output=True, text=True)
    assert (result.returncode, result.stdout.strip()) == (17, '-r'), result
    print('ok   dinit restart explicitly requests reboot and propagates failure')
    comm.write_text('init\n')
    result = subprocess.run(['bash', str(script)], capture_output=True, text=True)
    assert (result.returncode, result.stdout.strip()) == (17, '-r'), result
    print('ok   dinit invoked as init is recognized by its executable')
    init_exe.unlink()
    (root / 'runit').write_text('')
    init_exe.symlink_to(root / 'runit')
    comm.write_text('runit\n')
    result = subprocess.run(['bash', str(script)], capture_output=True, text=True)
    assert (result.returncode, result.stdout.strip()) == (18, 'reboot'), result
    print('ok   runit uses the absolute reboot command and propagates failure')
    reboot.unlink()
    result = subprocess.run(['bash', str(script)], capture_output=True, text=True)
    assert result.returncode == 1 and 'No reboot command' in result.stderr, result
    print('ok   missing reboot command reports a failure')
PY
