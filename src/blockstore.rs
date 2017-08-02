use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Seek, BufReader};
use std::fs::File;
use sha2::{Sha512, Digest};
use std::io::SeekFrom;
use readchain::{Take,Chain};
use std;

pub struct BlockStore {
    pub blocks: HashMap<String, Block>,
}

#[derive(Debug)]
pub struct Block {
    pub shards: Vec<BlockShard>,
    pub size: usize,
}

#[derive(Debug)]
pub struct BlockShard {
    pub file:    OsString,
    pub offset:  usize,
    pub size:    usize,
}

pub fn new() -> BlockStore {
    BlockStore{
        blocks: HashMap::new(),
    }
}


impl BlockStore {
    pub fn get<'a>(&'a self, hash: &String) -> Option<&'a Block> {
        self.blocks.get(hash)
    }
    pub fn insert(&mut self, hash: String, block: Block) -> bool {
        //sanity check on hash
        {
            let mut br = BufReader::new(block.chain());
            let hs = Sha512::digest_reader(&mut br).unwrap();
            let hs = format!("{:x}", hs);
            if hs != hash {

                let mut br = BufReader::new(block.chain());
                let mut content = Vec::new();
                let rs = br.read_to_end(&mut content).unwrap();

                if rs != block.size {
                    panic!(format!("BUG: block should be {} bytes but did read {}", block.size, content.len()));
                }


                let hs2 = Sha512::digest(&content);
                let hs2 = format!("{:x}", hs2);
                if hs2 != hs2 {
                    panic!("BUG: in chainreader: hash from read_to_end doesn't match digest_reader");
                }

                panic!(format!("BUG: inserted block hash id doesn't match its content. expected {} got {}", hash, hs));
            }
        }

        //collision check
        if self.blocks.contains_key(&hash) {
            let mut ra = BufReader::new(block.chain());
            let mut rb = BufReader::new(self.blocks[&hash].chain());
            loop {
                let mut a: [u8;1024] = [0; 1024];
                let mut b: [u8;1024] = [0; 1024];
                ra.read(&mut a).unwrap();
                let rs = rb.read(&mut b).unwrap();

                if a[..] != b[..] {
                    println!("!!!!!! HASH COLLISION !!!!!!!!!!!!!!!!!!!!!");
                    println!("this is extremly unlikely,save your block store for research.");
                    println!("{:?}", hash);
                    panic!("hash collision");
                }

                if rs < 1 {
                    break;
                }
            }
            return true;
        }
        self.blocks.insert(hash, block);
        return false;
    }

    pub fn load(&mut self, path: &str) {
        let entry_set = std::fs::read_dir(path).unwrap();
        for entry in entry_set {
            let entry = entry.unwrap();
            let entry_set2 = std::fs::read_dir(entry.path()).unwrap();
            for entry2 in entry_set2 {
                let entry2 = entry2.unwrap();
                let hash = entry.file_name().to_string_lossy().into_owned() + &entry2.file_name().to_string_lossy().into_owned();
                let size = entry2.metadata().unwrap().len() as usize;

                self.insert(hash, Block {
                    shards: vec![BlockShard{
                        file:    entry2.path().into_os_string(),
                        offset:  0,
                        size:    size,
                    }],
                    size: size,
                });
            }
        }
    }

}

impl Block {
    pub fn chain<'a>(&'a self) -> Chain<'a, Take<File>> {
        let it = self.shards.iter().map(|shard| {
            let mut f = File::open(&shard.file).unwrap();
            f.seek(SeekFrom::Current(shard.offset as i64)).unwrap();
            Take::limit(f, shard.size)
        });
        Chain::new(Box::new(it))
    }
}
