use std::fs::File;
use std::io::{Read, BufReader};
use rollsum;

use sha2::{Sha512, Digest};
use index::*;
use blockstore::{Block, BlockStore, BlockShard};
use pbr::ProgressBar;
use std::ffi::OsString;
use std::io::Stdout;


struct IntermediateBlockRef {
    inode:       u64,
    file_start:  usize, //where the file was when the block started
    file_end:    usize, //where the file completed inside the block
    block_start: usize, //where the block was when the file started
}


fn print_progress_bar(bar: &mut ProgressBar<Stdout>, path: &OsString){
    let mut s = path.to_str().unwrap();
    if s.len() > 40 {
        bar.message(&format!("indexing ..{:38} ", &s[s.len()-38..]));
    } else {
        bar.message(&format!("indexing {:40} ", &s));
    }
}

impl Index {
    fn emit_block(&mut self, blockstore: &mut BlockStore, len: usize, hash: Vec<u8>, inodes: &Vec<IntermediateBlockRef>) -> bool{

        let mut block_shards = Vec::new();
        //println!("block {}", hash);
        for ibr in inodes {
            //println!("   inode {} at offset {} is {} into the block with size {}",
            //         ibr.inode, ibr.file_start, ibr.block_start, ibr.file_end - ibr.file_start);
            block_shards.push(BlockShard{
                file:    self.inodes[ibr.inode as usize].host_path.clone(),
                offset:  ibr.file_start,
                size:    ibr.file_end - ibr.file_start,
            });

            if let None = self.inodes[ibr.inode as usize].c {
                self.inodes[ibr.inode  as usize].c = Some(Vec::new());
            }
            self.inodes[ibr.inode as usize].c.as_mut().unwrap().push(ContentBlockEntry{
                h: hash.clone(),
                o: ibr.block_start as u64,
                l: (ibr.file_end - ibr.file_start) as u64,
            });
        }

        blockstore.insert(hash, Block{
            shards: block_shards,
            size: len,
        })
    }

    pub fn serialize_to_blocks(&mut self, blockstore: &mut BlockStore) {
        let mut bar = ProgressBar::new(self.inodes.len() as u64);
        bar.show_speed = false;
        bar.show_time_left = false;

        let mut chunker = rollsum::Bup::new_with_chunk_bits(13);
        let mut hasher  = Sha512::default();

        let mut total_blocks = 0;
        let mut new_blocks = 0;

        let mut current_block_len = 0;
        let mut current_files_in_block = Vec::new();
        let mut current_file_pos = 0;


        let inodes = self.inodes.to_vec();
        for inode in inodes {
            bar.inc();
            if inode.k != 2 {
                continue;
            }
            print_progress_bar(&mut bar, &inode.host_path);


            let mut file = BufReader::new(File::open(&inode.host_path).unwrap());
            current_files_in_block.push(IntermediateBlockRef{
                inode: inode.i,
                file_start: 0,
                file_end:   0,
                block_start: current_block_len,
            });

            let mut buf = [0;1024];
            loop {
                let rs = file.read(&mut buf).unwrap();
                if rs < 1 {
                    break;
                }
                let mut restart = 0;

                loop {
                    if let Some(count) = chunker.find_chunk_edge(&buf[restart..rs]) {
                        current_block_len += count;
                        current_file_pos  += count;

                        current_files_in_block.last_mut().as_mut().unwrap().file_end = current_file_pos;

                        hasher.input(&buf[restart..restart+count]);
                        let hash = hasher.result().as_slice().to_vec();
                        hasher  = Sha512::default();

                        total_blocks +=1;
                        if !self.emit_block(blockstore, current_block_len, hash, &current_files_in_block) {
                            new_blocks += 1;
                        }
                        current_files_in_block.clear();
                        current_files_in_block.push(IntermediateBlockRef{
                            inode: inode.i,
                            file_start: current_file_pos,
                            file_end:   0,
                            block_start: 0,
                        });

                        current_block_len  = 0;
                        restart += count;
                    } else {
                        break;
                    }
                }
                hasher.input(&buf[restart..rs]);
                current_block_len += rs - restart;
                current_file_pos  += rs - restart;
            }
            current_files_in_block.last_mut().as_mut().unwrap().file_end = current_file_pos;
            current_file_pos = 0;
        }
        let hash = hasher.result().as_slice().to_vec();
        total_blocks += 1;
        if !self.emit_block(blockstore, current_block_len, hash, &current_files_in_block) {
            new_blocks += 1;
        }

        let total_inode_size = self.inodes.iter().fold(0, |acc, i| acc + i.s);
        bar.finish_print("");


        println!("done serializing {} inodes to {} blocks ({} new)",
                 self.inodes.len(), total_blocks, new_blocks);

    }
}
