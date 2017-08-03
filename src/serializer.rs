use blockstore::{Block, BlockStore, BlockShard};
use chunker::*;
use index::*;
use pbr::ProgressBar;
use readchain::{Take,Chain};
use serde::{Serialize, Deserialize};
use std::ffi::OsString;
use std::fs::File;
use std::io::{Stdout, Seek, SeekFrom, BufReader};
use std::path::Path;

impl Index {
    pub fn store_inodes(&mut self, blockstore: &mut BlockStore) {

        let mut bar = ProgressBar::new(self.i.len() as u64);
        let mut total_blocks = 0;
        let mut new_blocks = 0;

        let inodes = self.i.to_vec(); //TODO: only need to do this because borrow bla

        let it = inodes.iter().filter(|i|i.k == 2).map(|i| {
            (BufReader::new(File::open(&i.host_path).unwrap()), i.i)
        });

        let mut ci = Chunker::new(Box::new(it), ::rollsum::Bup::new(), 12);
        while let Some(c) = ci.next() {

            let mut block_shards = Vec::new();
            //println!("block {}", c.hash.to_hex());
            for ibr in c.parts {
                //println!("   inode {} at offset {} is {} into the block with size {}",
                //         ibr.i, ibr.file_start, ibr.block_start, ibr.file_end - ibr.file_start);

                block_shards.push(BlockShard{
                    file:    self.i[ibr.i as usize].host_path.clone(),
                    offset:  ibr.file_start,
                    size:    ibr.file_end - ibr.file_start,
                });

                if let None = self.i[ibr.i as usize].c {
                    self.i[ibr.i as usize].c = Some(Vec::new());
                }
                self.i[ibr.i as usize].c.as_mut().unwrap().push(ContentBlockEntry{
                    h: c.hash.clone(),
                    o: ibr.block_start as u64,
                    l: (ibr.file_end - ibr.file_start) as u64,
                });

                print_progress_bar(&mut bar, &self.i[ibr.i as usize].host_path);
                bar.set(ibr.i);
            }
            if blockstore.insert(c.hash, Block{
                shards: block_shards,
                size: c.len,
            }) {
                new_blocks += 1;
            }
            total_blocks += 1;
        }

        bar.finish();
        println!("done serializing {} inodes to {} blocks ({} new)",
                 self.i.len(), total_blocks, new_blocks);

    }


    pub fn store_index(&mut self, blockstore: &mut BlockStore) -> Index {
        //TODO that's probably shitty, but i can't be bothered to figure out passing an open file
        //to BlockShard right now
        let mut tmpindex = ::tempfile::NamedTempFile::new_in(".").unwrap();
        self.serialize(&mut ::rmps::Serializer::new(&mut tmpindex)).unwrap();
        tmpindex.seek(SeekFrom::Start(0)).unwrap();
        let path = OsString::from(tmpindex.path().to_str().unwrap());

        let tv= vec![tmpindex];
        let it = tv.iter().map(|i|(i,0));
        let mut ci = Chunker::new(Box::new(it), ::rollsum::Bup::new(), 12);

        let mut total_blocks = 0;
        let mut new_blocks = 0;

        let mut cbrs = Vec::new();
        while let Some(c) = ci.next() {
            let mut block_shards = Vec::new();
            //println!("block {}", c.hash.to_hex());
            for ibr in c.parts {
                //println!("   inode {} at offset {} is {} into the block with size {}",
                //         ibr.i, ibr.file_start, ibr.block_start, ibr.file_end - ibr.file_start);
                block_shards.push(BlockShard{
                    file:    path.clone(),
                    offset:  ibr.file_start,
                    size:    ibr.file_end - ibr.file_start,
                });
                cbrs.push(ContentBlockEntry{
                    h: c.hash.clone(),
                    o: ibr.block_start as u64,
                    l: (ibr.file_end - ibr.file_start) as u64,
                });
            }
            if blockstore.insert(c.hash, Block{
                shards: block_shards,
                size: c.len,
            }) {
                new_blocks += 1;
            }
            total_blocks += 1;
        }
        println!("done serializing index to {} blocks ({} new)", total_blocks, new_blocks);
        Index{
            v: 1,
            i: Vec::new(),
            c: Some(cbrs),
        }
    }

    pub fn load_index(&self, blockstore: &BlockStore) -> Index {
        let it = self.c.as_ref().unwrap().iter().map(|c| {
            let block = blockstore.get(&c.h).expect("block not found");
            let mut re = block.chain();
            re.seek(SeekFrom::Current(c.o as i64)).unwrap();
            Take::limit(re, c.l as usize)
        });
        let mut f = Chain::new(Box::new(it));
        Index::deserialize(&mut ::rmps::Deserializer::new(&mut f)).unwrap()
    }

    pub fn save_to_file(&mut self, path: &Path) {
        let mut f = File::create(path).unwrap();
        self.serialize(&mut ::rmps::Serializer::new(&mut f)).unwrap();
    }

    pub fn load_from_file(path: &Path) -> Index {
        let mut f = File::open(path).unwrap();
        Index::deserialize(&mut ::rmps::Deserializer::new(&mut f)).unwrap()
    }
}

fn print_progress_bar(bar: &mut ProgressBar<Stdout>, path: &OsString){
    let s = path.to_str().unwrap();
    if s.len() > 40 {
        bar.message(&format!("indexing ..{:38} ", &s[s.len()-38..]));
    } else {
        bar.message(&format!("indexing {:40} ", &s));
    }
}
