use blockstore::{BlockStore};
use fuse::*;
use index::{Index, Inode};
use libc::ENOENT;
use readchain::{Take,Chain};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use time::Timespec;
use std::boxed::Box;

const TTL: Timespec = Timespec { sec: 1, nsec: 0 };                 // 1 second

const CREATE_TIME: Timespec = Timespec { sec: 1381237736, nsec: 0 };    // 2013-10-08 08:56

fn entry_to_file_attr(entry: &Inode) -> FileAttr{
    FileAttr {
        ino: entry.i + 1,
        size: entry.s,
        blocks: entry.s * 512,
        atime: CREATE_TIME,
        mtime: CREATE_TIME,
        ctime: CREATE_TIME,
        crtime: CREATE_TIME,
        kind: match entry.k {
            1 => FileType::Directory,
            _ => FileType::RegularFile,
        },
        perm: entry.a,
        nlink: match entry.d {
            Some(ref d) => d.len() + 1,
            _ => 1,
        } as u32,
        uid: 1000,
        gid: 1000,
        rdev: 0,
        flags: 0,
    }
}


pub struct Fuse<'a> {
    index:      &'a Index,
    blockstore: &'a BlockStore,
    open_files:  HashMap<u64, Box<Read + 'a>>,
}

impl<'a> Fuse<'a> {
    pub fn new(index: &'a Index, blockstore: &'a BlockStore) -> Fuse<'a> {
        Fuse{
            index: index,
            blockstore: blockstore,
            open_files: HashMap::new(),
        }
    }
}

impl<'a>  Filesystem for Fuse<'a> {
    fn lookup (&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {

        let mb = self.index.i.get((parent - 1) as usize)
            .and_then(|entry| entry.d.as_ref())
            .and_then(|d| d.get(&name.to_string_lossy().into_owned()))
            .and_then(|e| self.index.i.get(e.i as usize));

        match mb {
            None => reply.error(ENOENT),
            Some(entry) => {
                let fa = &entry_to_file_attr(entry);
                reply.entry(&TTL, fa, 0)
            }
        }
    }

    fn getattr (&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        println!("getattr {:?}", ino);

        match self.index.i.get((ino - 1) as usize) {
            None => reply.error(ENOENT),
            Some(entry) => {
                reply.attr(&TTL, &entry_to_file_attr(entry));
            }
        }
    }


    fn open(&mut self, _req: &Request, ino: u64, _flags: u32, reply: ReplyOpen) {
        println!("open {:?}", ino);
        match self.index.i.get((ino - 1) as usize) {
            None => {reply.error(ENOENT);},
            Some(entry) => {
                let mut fh = entry.i;
                while self.open_files.contains_key(&fh) {
                    fh += 1;
                }
                self.open_files.insert(fh, Box::new(entry.chain(self.blockstore)));
                reply.opened(fh, 0);
            },
        };
    }
    fn release(&mut self,  _req: &Request, ino: u64, fh: u64,  _flags: u32, 
               _lock_owner: u64, _flush: bool, reply: ReplyEmpty) {
        println!("close {:?}", ino);
        self.open_files.remove(&fh);
        reply.ok();
    }

    fn read (&mut self, _req: &Request, ino: u64, fh: u64, offset: u64, size: u32, reply: ReplyData) {
        //TODO: i dont know if offset can be different than the last returned read size
        println!("read {:?} {} {}", ino, offset, size);

        let file = self.open_files.get_mut(&fh).unwrap();

        let mut buf = vec![0; size as usize];
        let r = file.read(&mut buf).unwrap();
        reply.data(&buf[..r]);
    }

    fn readdir (&mut self, _req: &Request, ino: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        println!("readdir {:?}", ino);
        if offset != 0 {
            reply.error(ENOENT);
            return;
        }
        match self.index.i.get((ino - 1) as usize) {
            None => reply.error(ENOENT),
            Some(entry) => {
                reply.add(1, 0, FileType::Directory, "."); //FIXME
                reply.add(1, 1, FileType::Directory, "..");

                let mut offset = 2;

                match entry.d {
                    None => reply.ok(),
                    Some(ref dir) => {
                        for (s,d) in dir {
                            reply.add(d.i, offset, match d.k {
                                1 => FileType::Directory,
                                _ => FileType::RegularFile,
                            }, s);
                            offset += 1;
                        }
                        reply.ok();
                    }
                };
            }
        }
    }
}

impl Inode {
    pub fn chain<'a>(&'a self, blockstore: &'a BlockStore) -> Chain<'a, Take<Chain<'a, Take<File>>>> {
        let c = self.c.as_ref().unwrap();
        let it = c.iter().map(move |c| {
            println!("reading from block {:?} offset  {} limit {}", c.h, c.o, c.l);

            let block = blockstore.get(&c.h).expect("block not found");
            let mut re = block.chain();
            re.seek(SeekFrom::Current(c.o as i64)).unwrap();
            Take::limit(re, c.l as usize)

        });
        Chain::new(Box::new(it))
    }
}
