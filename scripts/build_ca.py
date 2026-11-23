"""Install a user-supplied public CA into a staged Tizen filesystem."""
import argparse
import os
from pathlib import Path
import re
import subprocess
import tempfile


BUNDLE = Path('var/lib/ca-certificates/ca-bundle.pem')


def certificate():
    path = Path(os.environ['TIZEN_BUILD_CA_CERT']).resolve(strict=True)
    data = path.read_bytes().strip()
    if not re.fullmatch(rb'-----BEGIN CERTIFICATE-----\s+[A-Za-z0-9+/=\s]+'
                        rb'-----END CERTIFICATE-----', data):
        raise ValueError('TIZEN_BUILD_CA_CERT must contain one public PEM certificate')
    pem = subprocess.check_output(['openssl', 'x509', '-outform', 'PEM'], input=data)
    constraints = subprocess.check_output(['openssl', 'x509', '-noout', '-ext',
                                           'basicConstraints'], input=pem)
    if b'CA:TRUE' not in constraints:
        raise ValueError('TIZEN_BUILD_CA_CERT must be a CA certificate')
    return pem


def install_root(root, pem):
    bundle = root / 'opt' / BUNDLE
    if bundle.resolve(strict=True) != bundle or not bundle.is_file():
        raise ValueError(f'Unexpected Tizen CA bundle: {bundle}')
    with bundle.open('rb') as src:
        existing = src.read()
    if pem.strip() in existing:
        return
    with tempfile.TemporaryDirectory(dir=bundle.parent) as stage:
        tmp = Path(stage) / 'bundle'
        subprocess.run(['cp', '--preserve=all', bundle, tmp], check=True)
        with open(tmp, 'ab') as out:
            out.write(b'\n' + pem)
        # Keep the symlink in /etc/ssl and publish the new bundle atomically.
        os.replace(tmp, bundle)


def install_partition(image, pem):
    # ARM stores /opt on a separate ext4 partition. e2cp updates the existing
    # inode, retaining its extended attributes including security.SMACK64.
    dest = f'{image}:/{BUNDLE}'
    existing = subprocess.check_output(['e2cp', dest, '-'])
    if pem.strip() in existing:
        return
    stat = subprocess.check_output(['debugfs', '-R', f'stat /{BUNDLE}', image], text=True)
    mode = re.search(r'Type: regular\s+Mode:\s+(\d+)', stat)
    owner = re.search(r'User:\s+(\d+)\s+Group:\s+(\d+)', stat)
    if not mode or not owner:
        raise ValueError('Missing regular Tizen CA bundle in system-data')
    bundle = existing + b'\n' + pem
    subprocess.run(['e2cp', '-O', owner[1], '-G', owner[2], '-P', mode[1],
                    '-', dest], input=bundle, check=True)
    if subprocess.check_output(['e2cp', dest, '-']) != bundle:
        raise ValueError('Could not verify the updated Tizen CA bundle')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    target = parser.add_mutually_exclusive_group(required=True)
    target.add_argument('--export', type=Path, help='write the validated public CA in PEM format')
    target.add_argument('--root', type=Path)
    target.add_argument('--system-data', type=Path)
    args = parser.parse_args()
    if os.environ.get('TIZEN_BUILD_CA_CERT'):
        try:
            pem = certificate()
            if args.export:
                args.export.write_bytes(pem)
            elif args.root:
                install_root(args.root.resolve(), pem)
            else:
                install_partition(args.system_data, pem)
        except (ValueError, OSError, subprocess.CalledProcessError) as err:
            parser.exit(1, f'TIZEN_BUILD_CA_CERT: {err}\n')
