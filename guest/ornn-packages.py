"""Repository settings and local RPM selection for the guest ornn CLI."""
from collections import defaultdict
import configparser
import fcntl
import hashlib
import os
from pathlib import Path
import re
import sys
from urllib.parse import urlsplit


def repositories(src, dst):
    urls = []
    section = None
    for number, line in enumerate(src.read_text().splitlines(), 1):
        line = line.strip()
        if not line or line.startswith(('#', ';')):
            continue
        if line == '[repos]':
            section = line
            continue
        key, sep, url = line.partition('=')
        url = url.strip()
        if (section != '[repos]' or key.strip() != 'url' or not sep
                or urlsplit(url).scheme not in ('http', 'https') or not urlsplit(url).hostname
                or any(c.isspace() for c in url)):
            sys.exit(f'{src}:{number}: expected url = https://server/repository/')
        if url not in urls:
            urls.append(url)
    if not urls:
        sys.exit(f'{src}: [repos] needs at least one URL')

    def read_repos(directory):
        config = configparser.ConfigParser(interpolation=None, strict=False)
        config.read(sorted(directory.glob('*.repo')))
        return config

    system = read_repos(Path('/etc/zypp/repos.d'))
    snapshot = read_repos(Path('/usr/share/tizen-snapshot/repos.d'))
    # The image carries a countersigned copy of its pinned snapshot indexes.
    pinned = {system[a].get('baseurl', '').rstrip('/'): a
              for a in ('tizen-base', 'tizen-emulator') if a in system and a in snapshot}
    seed = dst.parents[1] / 'seed' / 'ornn.repo'
    with dst.open('w') as live, seed.open('w') as initial:
        for priority, url in enumerate(urls, 2):
            alias = pinned.get(url.rstrip('/'))
            seed_url = snapshot[alias]['baseurl'] if alias else url
            refresh = 0 if alias else 1
            alias = alias or 'ornn-' + hashlib.sha256(url.encode()).hexdigest()[:20]
            for out, source in ((live, url), (initial, seed_url)):
                print(f'[{alias}]\nbaseurl={source}\nenabled=1\nautorefresh={refresh}\n'
                      f'type=rpm-md\npriority={priority}\ngpgcheck=1\n', file=out)


def packages(root, arch, scratch, wanted):
    import rpm
    database = rpm.TransactionSet()
    catalog = defaultdict(list)
    providers = defaultdict(list)
    local = Path(scratch) / 'local'
    local.mkdir()

    for target in sorted(Path(root).resolve().glob(f'*/{arch}')):
        if not target.is_dir() or target.is_symlink():
            continue
        # Windows-backed 9P needs a move-aside step to replace a closed file.
        # Hold the same directory lock as the publisher while reading the pointer.
        fd = os.open(target, os.O_RDONLY | os.O_DIRECTORY)
        try:
            fcntl.flock(fd, fcntl.LOCK_SH)
            marker = target / '.last-success'
            if not marker.is_file():
                continue
            run = marker.read_text().strip()
        finally:
            os.close(fd)
        if not re.fullmatch(r'run\.[a-zA-Z0-9]+', run):
            sys.exit(f'Invalid build record: {marker}')
        top = target / run
        built = dict(line.split() for line in (top / 'built').read_text().splitlines())
        for path in sorted((top / 'RPMS').glob('*/*.rpm')):
            with path.open('rb') as package:
                header = database.hdrFromFdno(package)
            name = header['name']
            identity = f"{name}.{header['arch']}"
            if identity not in built:
                continue
            digest = header['sigmd5'].hex()
            if built.pop(identity) != digest:
                sys.exit(f'Local RPM differs from its completed build: {path}')
            if header['arch'] not in (arch, 'noarch'):
                continue
            catalog[name].append((path, header))
            link = local / path.name
            if not link.exists():
                link.symlink_to(path)
            for capability in header.dsFromHeader(rpm.RPMTAG_PROVIDENAME):
                providers[capability.N()].append((capability, name, top))
        if built:
            sys.exit(f'Missing local build RPMs under {top}: {", ".join(built)}')

    seen = set()
    while wanted:
        name = wanted.pop(0)
        if name in seen:
            continue
        seen.add(name)
        matches = catalog.get(name, [])
        if len(matches) > 1:
            sys.exit(f'Multiple local RPMs provide {name}. Clean the obsolete build first.')
        installed = list(database.dbMatch('name', name))
        if not matches:
            if not any(h['arch'] in (arch, 'noarch') for h in installed):
                print(name)
            continue
        path, header = matches[0]
        if not any(h['arch'] == header['arch'] and h['sigmd5'] == header['sigmd5']
                   for h in installed):
            print(path)
        # A version match alone can leave headers with an older local library.
        for requirement in header.dsFromHeader(rpm.RPMTAG_REQUIRENAME):
            candidates = [(n, top) for cap, n, top in providers[requirement.N()]
                          if requirement.Compare(cap)]
            same_build = {n for n, top in candidates if top == path.parents[2]}
            selected = same_build or {n for n, _ in candidates}
            if len(selected) == 1:
                wanted.extend(selected)


if __name__ == '__main__':
    if sys.argv[1] == 'repositories':
        repositories(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == 'packages':
        packages(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5:])
    else:
        sys.exit('Unknown Ornn helper command')
