not usable yet, stay tuned.

if you insist:

```
$ cargo build --release
$ ./target/release/korhal-image store-init file:///tmp/store/
$
$ ./target/release/korhal-image push . file:///tmp/store/funnyname     
loading content from /tmp/store/content
indexing ..270af54aea45c008b00ab84f65481aa74cd6b8 19921 / 19921 [====================================================================================================================================================================================================] 100.00 % 2938.31/s
done serializing 19921 inodes to 123452 blocks (48987 new)
done serializing index to 2632 blocks (2231 new)
done serializing index to 35 blocks (35 new)
done serializing index to 1 blocks (1 new)
input stored into index 72e7a203ed854720e1023f95a48990a0a278d9057c135b64f1d04945a69766d4 with name "funnyname"
$
$ mkdir /tmp/mnt
$ ./target/release/korhal-image mount file:///tmp/store/funnyname /tmp/bla 
loading content from /tmp/store/content
mounting index "funnyname" with 19922 inodes to /tmp/bla

$ ls /tmp/bla
$ Cargo.toml src target test ...

```
