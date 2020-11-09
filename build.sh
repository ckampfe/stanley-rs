#!/usr/bin/env sh
set -euxo pipefail

RUSTFLAGS="-C target-cpu=native" cargo install --path .
echo $(which stanley-rs)
stanley-rs
echo "tar -cvf build.tar build"
tar -cvf build.tar build
echo "GZIP=-9 pigz build.tar"
GZIP=-9 pigz build.tar
echo "mv build.tar.gz build.tgz"
mv build.tar.gz build.tgz
echo "scp build.tgz ckampfe@zeroclarkthirty.com:~/build.tgz"
scp build.tgz ckampfe@zeroclarkthirty.com:~/build.tgz
echo "Copied build.tgz to ckampfe@zeroclarkthirty.com"
echo "ssh ckampfe@zeroclarkthirty.com 'tar -xvf build.tgz; sudo cp -r build/* /usr/share/nginx/www'"
ssh -t ckampfe@zeroclarkthirty.com 'tar -xvf build.tgz; sudo cp -r build/* /usr/share/nginx/www'
