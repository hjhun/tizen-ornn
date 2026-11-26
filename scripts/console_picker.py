"""Choose among live Ornn tmux consoles on the local host."""
import os
from pathlib import Path
import subprocess


def running_consoles():
    """Find this user's attachable Ornn panes across image stores."""
    uid = os.getuid()
    runtime = Path(f'/tmp/tizen-ornn-{uid}')
    if (not runtime.is_dir() or runtime.is_symlink() or
            runtime.stat().st_uid != uid or runtime.stat().st_mode & 0o077):
        return []
    choices = []
    for socket in runtime.glob('*.sock'):
        if not socket.is_socket() or socket.is_symlink():
            continue
        pane = subprocess.run(['tmux', '-S', socket, 'list-panes', '-t', '=vm:console',
                               '-F', '#{pane_pid}'], capture_output=True, text=True)
        pid = pane.stdout.strip()
        if pane.returncode or not pid.isdecimal():
            continue
        try:
            argv = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
        except (FileNotFoundError, PermissionError):
            continue
        if b'--pane' not in argv:
            continue
        args = argv[argv.index(b'--pane') + 1:]
        if not args or os.fsdecode(args[0]) != str(socket.with_suffix('.exit')):
            continue
        disk = next((os.fsdecode(a[len(b'TIZEN_DISK='):]) for a in args
                     if a.startswith(b'TIZEN_DISK=')), None)
        if disk:
            choices.append((Path(disk), socket))
    return sorted(choices)


def attach_console(socket):
    os.environ.pop('TMUX', None)
    os.environ.pop('TMUX_PANE', None)
    os.execvp('tmux', ['tmux', '-S', str(socket), 'attach-session', '-t', '=vm'])


def choose_image(choices, action, prompt_single=False, full_path=False):
    if len(choices) == 1 and not prompt_single:
        return choices[0]
    print(f'Select a Tizen Ornn VM image to {action}:')
    for index, (arch, image) in enumerate(choices, 1):
        print(f'  {index}. {arch}  {image if full_path else image.name}')
    while True:
        try:
            answer = input('Image [1], q to cancel: ').strip()
        except EOFError:
            return None
        if answer.lower() == 'q':
            return None
        if not answer:
            return choices[0]
        for index, choice in enumerate(choices, 1):
            if answer == str(index):
                return choice
        print(f'Enter a number from 1 to {len(choices)}.')


def pick_running():
    consoles = running_consoles()
    if len(consoles) < 2:
        return False
    choices = [('x86_64', disk) for disk, _ in consoles]
    choice = choose_image(choices, 'attach to (running)', full_path=True)
    if choice is not None:
        attach_console(dict(consoles)[choice[1]])
    return True
