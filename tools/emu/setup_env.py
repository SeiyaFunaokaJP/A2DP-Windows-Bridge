"""Create the Python virtual environment for tools/emu (Windows, Python 3.11+).

    python tools/emu/setup_env.py

Creates tools/emu/.venv and installs requirements.txt (Bumble and its dependencies, all pinned).
Safe to run again: an existing environment is reused and brought up to date.
"""

import os
import subprocess
import sys
import venv

HERE = os.path.dirname(os.path.abspath(__file__))
VENV_DIR = os.path.join(HERE, '.venv')


def venv_python():
    return os.path.join(VENV_DIR, 'Scripts', 'python.exe')


def main():
    if sys.version_info < (3, 11):
        sys.exit(f'Python 3.11 or later is required (this is {sys.version.split()[0]})')
    if not os.path.exists(venv_python()):
        print(f'Creating virtual environment: {VENV_DIR}')
        venv.EnvBuilder(with_pip=True).create(VENV_DIR)
    print('Installing requirements...')
    subprocess.check_call([venv_python(), '-m', 'pip', 'install', '--quiet', '--disable-pip-version-check',
                           '-r', os.path.join(HERE, 'requirements.txt')])
    version = subprocess.check_output(
        [venv_python(), '-c', 'import importlib.metadata as m; print(m.version("bumble"))'], text=True).strip()
    print(f'Ready: Bumble {version} in {VENV_DIR}')
    print('Next: build A2DPWB and a2dpwb_decode (Release), then run  python tools/emu/run_test.py')


if __name__ == '__main__':
    main()
