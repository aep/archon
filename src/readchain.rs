use std::iter::Iterator;
use std::io::{Result, Read, Seek, SeekFrom, Error, ErrorKind};
use std::cmp;

/// like std::io::Take but with Seek
pub struct Take<R>  where R: Read {
    inner: R,
    limit: usize,
}

impl<R> Take<R> where R: Read{
    pub fn limit(r: R, limit: usize) -> Take<R> {
        Take{
            inner: r,
            limit: limit,
        }
    }
}

impl<R> Read for Take<R> where R: Read{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // Don't call into inner reader at all at EOF because it may still block
        if self.limit == 0 {
            return Ok(0);
        }

        let max = cmp::min(buf.len(), self.limit) as usize;
        let n = self.inner.read(&mut buf[..max])?;
        self.limit -= n;
        Ok(n)
    }
}

impl<R> Seek for Take<R> where R: Read+Seek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::End(_) | SeekFrom::Start(_) => {
                return Err(Error::new(ErrorKind::NotFound, "cannot seek end/start on Take"));
            },
            SeekFrom::Current(seek) => {
                self.inner.seek(SeekFrom::Current(cmp::min(seek, self.limit as i64)))
            }
        }
    }
}


/// like std::io::Chain but on an Iterator which may contain a lambda and with Seek
pub struct Chain<'a, R> where R : Read {
    it: Box<Iterator<Item=R> + 'a>,
    cur: Option<R>,
}


impl<'a, R> Chain<'a, R> where R : Read {
    pub fn new(it: Box<Iterator<Item=R> + 'a>) -> Chain<'a, R>{
        Chain{
            it: it,
            cur: None,
        }
    }
}

impl<'a, R> Read for Chain<'a, R> where R : Read {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut didread = 0;
        loop {
            if let None = self.cur {
                match self.it.next() {
                    None => return Ok(didread),
                    Some(r) => {
                        self.cur = Some(r);
                    }
                }
            }
            let n = buf.len() - didread;
            match self.cur.as_mut().unwrap().read(&mut buf[didread..(didread+n)]) {
                Err(e) => return Err(e),
                Ok(rs) => {
                    didread += rs;
                    if didread >= buf.len(){
                        return Ok(didread);
                    }
                    self.cur = None;
                }
            }
        }
    }
}

impl<'a, R> Seek for Chain<'a, R> where R : Read + Seek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::End(_) | SeekFrom::Start(_) => {
                return Err(Error::new(ErrorKind::NotFound, "cannot seek on iterator"));
            },
            SeekFrom::Current(start) => {
                if start < 0 {
                    return Err(Error::new(ErrorKind::NotFound, "cannot seek backwards on iterator"));
                }
                let mut seeked = 0 as i64;
                loop {
                    if let None = self.cur {
                        match self.it.next() {
                            None => return Ok(seeked as u64),
                            Some(r) => {
                                self.cur = Some(r);
                            }
                        }
                    }

                    let n = start - seeked;
                    match self.cur.as_mut().unwrap().seek(SeekFrom::Current(n)) {
                        Err(e) => return Err(e),
                        Ok(rs) => {
                            seeked += rs as i64;
                            if seeked >= start {
                                return Ok(seeked as u64);
                            }
                            self.cur = None;
                        }
                    }
                }
            },
        };
    }
}


#[cfg(test)]
use std::fs::File;

#[test]
fn some_files() {

    let files = vec![
        ("test/readchain/a", 0, 4),
        ("test/readchain/b", 0, 4),
    ].into_iter().map(|(f,o,l)| {
        let mut f = File::open(f).unwrap();
        f.seek(SeekFrom::Start(o)).unwrap();
        f.take(l as u64)
    });

    let mut content = String::new();
    Chain::new(Box::new(files)).read_to_string(&mut content).unwrap();
    assert_eq!(content, "yayacool");
}

#[test]
fn overshoot() {

    let files = vec![
        ("test/readchain/a", 0, 4123123213),
    ].into_iter().map(|(f,o,l)| {
        let mut f = File::open(f).unwrap();
        f.seek(SeekFrom::Start(o)).unwrap();
        f.take(l)
    });

    let mut content = String::new();
    let mut rr = Chain::new(Box::new(files));
    let mut void = [0;2];
    rr.read(&mut void).unwrap();
    rr.read_to_string(&mut content).unwrap();
    assert_eq!(content, "ya");
}

#[test]
fn overslimit() {

    let files = vec![
        ("test/readchain/a", 0, 3),
    ].into_iter().map(|(f,o,l)| {
        let mut f = File::open(f).unwrap();
        f.seek(SeekFrom::Start(o)).unwrap();
        f.take(l)
    });

    let mut content = String::new();
    let mut rr = Chain::new(Box::new(files));
    let mut void = [0;2];
    rr.read(&mut void).unwrap();
    rr.read_to_string(&mut content).unwrap();
    assert_eq!(content, "y");
}

#[test]
fn nested() {

    let cl = |(f,o,l)| {
        let mut f = File::open(f).unwrap();
        f.seek(SeekFrom::Start(o)).unwrap();
        Take::limit(f, l)
    };

    let fa = vec![
        ("test/readchain/a", 0, 4),
        ("test/readchain/a", 0, 4),
    ].into_iter().map(&cl);

    let fb = vec![
        ("test/readchain/b", 0, 10),
        ("test/readchain/b", 0, 10),
    ].into_iter().map(&cl);

    let files = vec![
        (fa, 0, 4),
        (fb, 4, 6),
    ].into_iter().map(|(f,o,l)| {
        let mut f = Chain::new(Box::new(f));
        f.seek(SeekFrom::Current(o)).unwrap();
        f.take(l)
    });
    let mut content = String::new();
    Chain::new(Box::new(files)).read_to_string(&mut content).unwrap();
    assert_eq!(content, "yaya stuff");
}


#[cfg(test)]
pub struct BlockShard {
    file:    String,
    offset:  usize,
    size:    usize,
}

#[cfg(test)]
struct Block {
    shards: Vec<BlockShard>,
}

#[cfg(test)]
impl<'a> Block {
    fn chain(&'a self) -> Chain<'a, Take<File>> {
        let it = self.shards.iter().map(|shard| {
            let mut f = File::open(&shard.file).unwrap();
            f.seek(SeekFrom::Current(shard.offset as i64)).unwrap();
            Take::limit(f, shard.size)
        });
        Chain::new(Box::new(it))
    }
}

#[test]
fn block() {
    let bl = Block{
        shards: vec![
            BlockShard{file: String::from("test/readchain/a"), offset: 0, size:4},
            BlockShard{file: String::from("test/readchain/b"), offset: 0, size:4},
        ]
    };
    let mut content = String::new();
    bl.chain().read_to_string(&mut content).unwrap();
    assert_eq!(content, "yayacool");
}
