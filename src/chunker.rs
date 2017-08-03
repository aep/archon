use std::io::{Read, BufReader};
use sha2::{Sha512, Digest};

/// takes an iterator over tuple (Read, I)
/// and provides an iterator over Chunk{hash, parts<I>}
///
pub struct Chunker<'a, R, C, I> where R : Read, C: ::rollsum::Engine {
    it: Box<Iterator<Item=(R, I)> + 'a>,
    current_read: Option<(R,I)>,
    current_parts: Vec<ChunkPart<I>>,
    current_block_len: usize,
    current_file_pos: usize,

    chunker: C,
    bits: u32,

    hasher: Sha512,

    buf: [u8;4096],
    buflen : usize,
    bufpos : usize,
    bufsincelastblock: usize,
}

pub struct Chunk<I> {
    pub len:  usize,
    pub hash: Vec<u8>,
    pub parts: Vec<ChunkPart<I>>
}

pub struct ChunkPart<I> {
    pub i:       I,
    pub file_start:  usize, //where the file was when the block started
    pub file_end:    usize, //where the file completed inside the block
    pub block_start: usize, //where the block was when the file started
}

impl<'a, R, C, I> Chunker<'a, R, C, I> where I: Copy, R: Read, C: ::rollsum::Engine {
    pub fn new(it: Box<Iterator<Item=(R, I)> + 'a>, c: C, bits: u32) -> Chunker<'a, R, C, I>{
        Chunker{
            it: it,
            current_read: None,
            current_parts: Vec::new(),
            current_block_len: 0,
            current_file_pos: 0,

            chunker: c,
            bits: bits,

            hasher: Sha512::default(),

            buf: [0;4096],
            buflen: 0,
            bufpos: 0,
            bufsincelastblock: 0,
        }
    }

    fn fill(&mut self) -> bool {
        if let None = self.current_read {
            match self.it.next() {
                None => return false,
                Some(r) => {
                    self.current_parts.push(ChunkPart{
                        i: r.1,
                        file_start: 0,
                        file_end:   0,
                        block_start: self.current_block_len,
                    });
                    self.current_file_pos = 0;
                    self.current_read = Some(r);
                }
            }
        }
        match self.current_read.as_mut().unwrap().0.read(&mut self.buf) {
            Err(e) => panic!(e),
            Ok(some) => {
                if some < 1 {
                    self.current_parts.last_mut().as_mut().unwrap().file_end = self.current_file_pos;
                    self.current_read = None;
                    return self.fill();
                } else {
                    self.buflen = some;
                    return true;
                }
            }
        }
    }
}


impl<'a, R, C, I> Iterator for Chunker<'a, R, C, I> where I: Copy, R: Read, C: ::rollsum::Engine<Digest = u32> {
    type Item = Chunk<I>;
    fn next(&mut self) -> Option<Self::Item> {
        let chunk_mask = (1 << self.bits) - 1;
        loop {
            if self.bufpos >= self.buflen {

                self.current_block_len += self.bufpos-self.bufsincelastblock;
                self.current_file_pos  += self.bufpos-self.bufsincelastblock;
                self.hasher.input(&self.buf[self.bufsincelastblock..self.bufpos]);

                self.bufsincelastblock = 0;
                self.bufpos = 0;
                self.buflen = 0;

                if !self.fill() {
                    //rest
                    if self.current_parts.len() > 0 {
                        let hash = self.hasher.result().as_slice().to_vec();
                        self.current_parts.last_mut().as_mut().unwrap().file_end = self.current_file_pos;
                        return Some(Chunk{
                            len: self.current_block_len,
                            hash: hash,
                            parts: ::std::mem::replace(&mut self.current_parts, Vec::new()),
                        });
                        self.bufsincelastblock = self.bufpos;
                    } else {
                        debug_assert!(self.bufsincelastblock == 0 && self.bufpos == 0, "end of iterator with leftover bytes");
                        return None;
                    }
                }
            }
            debug_assert!(self.current_parts.len() > 0, format!(
                    "continuing to iterate when current_parts is empty. bufpos: {}, buflen: {}", self.bufpos, self.buflen));

            self.chunker.roll_byte(self.buf[self.bufpos]);
            self.bufpos += 1;

            if self.chunker.digest() & chunk_mask == chunk_mask {

                self.current_block_len += self.bufpos-self.bufsincelastblock;
                self.current_file_pos  += self.bufpos-self.bufsincelastblock;
                self.hasher.input(&self.buf[self.bufsincelastblock..self.bufpos]);

                let hash = self.hasher.result().as_slice().to_vec();
                self.hasher = Sha512::default();

                self.current_parts.last_mut().as_mut().unwrap().file_end = self.current_file_pos;
                let rr = Chunk{
                    len: self.current_block_len,
                    hash: hash,
                    parts: ::std::mem::replace(&mut self.current_parts, Vec::new()),
                };
                self.current_parts.push(ChunkPart{
                    i: self.current_read.as_ref().unwrap().1,
                    file_start: self.current_file_pos,
                    file_end:   0,
                    block_start: 0,
                });

                self.current_block_len = 0;
                self.bufsincelastblock = self.bufpos;

                return Some(rr);
            }
        }
        None
    }
}

