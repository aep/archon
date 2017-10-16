use blockstore::{Block, BlockStore, BlockShard};
use chunker::*;
use index::*;
use pbr::ProgressBar;
use readchain::{Take,Chain};
use serde::{Serialize, Deserialize};
use std::ffi::OsString;
use std::io::{Stdout, Seek, SeekFrom, BufReader};
use std::path::Path;
use std::fs::File;

use elfkit;
use sha2::{Sha256, Digest};
use std::io::Read;

macro_rules! kb_fmt {
    ($n: ident) => {{
        let kb = 1024f64;
        match $n as f64{
            $n if $n >= kb.powf(4_f64) => format!("{:.*} TB", 2, $n / kb.powf(4_f64)),
            $n if $n >= kb.powf(3_f64) => format!("{:.*} GB", 2, $n / kb.powf(3_f64)),
            $n if $n >= kb.powf(2_f64) => format!("{:.*} MB", 2, $n / kb.powf(2_f64)),
            $n if $n >= kb => format!("{:.*} KB", 2, $n / kb),
            _ => format!("{:.*} B", 0, $n)
        }
    }}
}

impl Index {
    pub fn store_inodes(&mut self, blockstore: &mut BlockStore) {

        let total_bytes = self.i.iter().fold(0, |acc, ref x| acc + x.size);

        let mut bar = ProgressBar::new(total_bytes);
        bar.set_units(::pbr::Units::Bytes);

        let mut new_bytes  = 0;
        let mut new_blocks = 0;
        let mut total_blocks = 0;


        let mut inodes = self.i.to_vec();

        // detect special files
        for i in &mut inodes {
            if i.kind != 2 {
                continue;
            }
            let mut host_file  = File::open(&i.host_path).unwrap();
            let cuts = match elfkit::Elf::from_reader(&mut host_file) {
                Err(_) => None,
                Ok(mut elf) => {
                    let mut r = None;
                    for sec in elf.sections.drain(..) {
                        if sec.header.shtype == elfkit::types::SectionType(0x6fffff01) {
                            let mut rr = Vec::new();
                            let mut io = &sec.content.into_raw().unwrap()[..];
                            while let Ok(o) = elf_read_u32!(&elf.header, io) {
                                rr.push(o as usize);
                            }
                            r = Some(rr);
                        }
                    }
                    r
                }
            };
            match cuts {
                None => {},
                Some(mut cuts) => {
                    i.kind = 3;
                    let mut at = 0;
                    cuts.push(host_file.metadata().unwrap().len() as usize);
                    cuts.sort_unstable();
                    host_file.seek(SeekFrom::Start(0));
                    for cut in cuts {
                        let mut buf = vec![0;cut - at];
                        host_file.read_exact(&mut buf).unwrap();
                        let hash = Sha256::digest(&buf).as_slice().to_vec();
                        if blockstore.insert(hash.clone(), Block {
                            shards: vec![BlockShard{
                                file:    i.host_path.clone(),
                                offset:  at,
                                size:    buf.len(),
                            }],
                            size: buf.len(),
                        }) {
                            new_blocks +=1;
                            new_bytes  += buf.len();
                        }
                        total_blocks += 1;
                        bar.add(buf.len() as u64);

                        if let None = self.i[i.inode as usize].content {
                            self.i[i.inode as usize].content = Some(Vec::new());
                        }
                        self.i[i.inode as usize].content.as_mut().unwrap().push(ContentBlockEntry{
                            h: hash,
                            o: 0,
                            l: buf.len() as u64,
                        });

                        at = cut;
                    }
                    let mut buf = Vec::new();
                    assert!(host_file.read_to_end(&mut buf).unwrap() == 0);


                }
            }
        }



        let it = inodes.iter().filter(|i|i.kind == 2).map(|i| {
            (BufReader::new(File::open(&i.host_path).unwrap()), i.inode)
        });

        let mut ci = Chunker::new(Box::new(it), ::rollsum::Bup::new(), 9);
        while let Some(c) = ci.next() {
            bar.add((c.len) as u64);

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

                if let None = self.i[ibr.i as usize].content {
                    self.i[ibr.i as usize].content = Some(Vec::new());
                }
                self.i[ibr.i as usize].content.as_mut().unwrap().push(ContentBlockEntry{
                    h: c.hash.clone(),
                    o: ibr.block_start as u64,
                    l: (ibr.file_end - ibr.file_start) as u64,
                });
                print_progress_bar(&mut bar, &self.i[ibr.i as usize].host_path);
            }
            if blockstore.insert(c.hash, Block{
                shards: block_shards,
                size: c.len,
            }) {
                new_blocks +=1;
                new_bytes  += c.len;
            }
            total_blocks += 1;
        }

        bar.finish();
        println!("done indexing {} inodes to {} blocks", self.i.len(), total_blocks);
        println!(" + {} blocks {}", new_blocks, kb_fmt!(new_bytes));
    }


    pub fn store_index(&mut self, blockstore: &mut BlockStore) -> Index {
        //TODO used a namedtempfile isnt great,
        //but i can't be bothered to figure out passing a &File to BlockShard right now
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
    if s.len() > 50 {
        bar.message(&format!("indexing ..{:48} ", &s[s.len()-48..]));
    } else {
        bar.message(&format!("indexing {:50} ", &s));
    }
}


