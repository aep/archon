not usable yet, stay tuned.

if you insist:


```
$ cargo build --release
$ export KORHAL_STORE=/tmp/store
$ ./target/release/korhal-image store . myspace
loading content from /tmp/store/content
done serializing 19921 inodes to 123452 blocks (48987 new)
done serializing index to 2632 blocks (2231 new)
done serializing index to 35 blocks (35 new)
done serializing index to 1 blocks (1 new)
input stored into index .. with name "myspace"
$
$ mkdir /tmp/mnt
$ ./target/release/korhal-image mount myspace /tmp/bla
loading content from /tmp/store/content
mounting index "myspace" with 19922 inodes to /tmp/bla

$ ls /tmp/bla
$ Cargo.toml src target test ...

```


