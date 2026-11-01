import package_version
from subprocess import PIPE, Popen

version = package_version.get_version()

p = Popen(
    f"cargo info closure-ffi@{version} --registry=crates-io", 
    shell=True, stdout=PIPE, stderr=PIPE
)

stdout, stderr = p.communicate()

if p.returncode == 0:
    print(f"error: closure-ffi {version} already exists on crates.io")
    exit(1)
elif b"error: could not find" not in stderr:
    print("error: failed to query crates.io registry:\n")
    print(stderr)
    exit(1)
