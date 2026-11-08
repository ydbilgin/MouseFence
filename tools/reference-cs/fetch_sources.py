"""Extract the pinned 1.1.1 reference inputs from git, with file hashes."""
import hashlib
import io
from pathlib import Path, PurePosixPath
import subprocess
import tarfile


def main():
    repo = Path(__file__).resolve().parents[2]
    subprocess.run(['git', '-C', str(repo), 'rev-parse', '--verify', 'refs/tags/v1.1.1'],
                   check=True, stdout=subprocess.DEVNULL)
    archive = subprocess.run(
        ['git', '-C', str(repo), 'archive', '--format=tar', 'v1.1.1',
         'Core', 'Settings.cs', 'Platform/Native.cs', 'I18n'],
        check=True, capture_output=True).stdout
    dest = repo / 'reference-src' / 'v1.1.1'
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar:
            name = PurePosixPath(member.name)
            if name.is_absolute() or '..' in name.parts or not (member.isfile() or member.isdir()):
                raise ValueError('unsafe reference archive member')
            path = dest.joinpath(*name.parts)
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
                continue
            data = tar.extractfile(member).read()
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            print(f'{hashlib.sha256(data).hexdigest()}  {name}')


if __name__ == '__main__':
    main()
