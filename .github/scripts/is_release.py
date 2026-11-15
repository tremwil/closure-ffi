import package_version
import sys
from subprocess import PIPE, Popen

version = package_version.get_version()

p = Popen(
    f"cargo info closure-ffi@{version} --registry=crates-io", 
    shell=True, stdout=PIPE, stderr=PIPE
)

stdout, stderr = p.communicate()

if p.returncode == 0:
    print("is_release=false")
elif b"error: could not find" not in stderr:
    print("error: failed to query crates.io registry:\n", file=sys.stderr)
    print(stderr, file=sys.stderr)
    exit(1)
else:
    print("is_release=true")
