use serde::{Serialize, Serializer};
use std::collections::{HashMap, BTreeMap};

#[derive(Serialize, Deserialize, Clone)]
pub struct Inode {
    pub i: u64,     //inode (might later use this as offset into the binary formated index)
    pub p: u64,     //parent
    pub s: u64,     //size
    pub k: u16,     //kind
    pub a: u16,     //perms

    #[serde(serialize_with = "ordered_map")]
    pub d: Option<HashMap<String, ContentDirEntry>>, //directory
    pub h: Option<String>, //file hash
    pub c: Option<Vec<ContentBlockEntry>>, //content blocks

    #[serde(skip)]
    pub host_path: ::std::ffi::OsString, // full path. will not be stored
}

fn ordered_map<S>(value: &Option<HashMap<String, ContentDirEntry>>, serializer: S) -> Result<S::Ok, S::Error>
where S: Serializer
{
    match *value {
        Some(ref val) => {
            let ordered: BTreeMap<_, _> = val.iter().collect();
            ordered.serialize(serializer)
        },
        None => {
            let fake : Option<HashMap<String, ContentDirEntry>> = None;
            fake.serialize(serializer)
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContentBlockEntry {
    pub h: Vec<u8>,  //block hash
    pub o: u64,     //offset into block
    pub l: u64,     //length into block
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContentDirEntry {
    pub i: u64,     //inode
    pub k: u16,     //kind
}

#[derive(Serialize, Deserialize)]
pub struct Index {
    pub v: u16, //version
    pub i: Vec<Inode>, //inodes. i or c cannot exist at the same time
    pub c: Option<Vec<ContentBlockEntry>>, //content blocks that compose another index
}

fn collect_dir(path: ::std::ffi::OsString) -> ::std::io::Result<Vec<::std::fs::DirEntry>> {
    let entry_set = try!(::std::fs::read_dir(path));
    let mut entries = try!(entry_set.collect::<Result<Vec<_>, _>>());
    entries.sort_by(|a, b| a.path().cmp(&b.path()));
    Ok(entries)
}

impl Index {
    fn add_from_dir_entry(&mut self, parent_inode: u64, path: ::std::fs::DirEntry) -> (String, ContentDirEntry) {
        let meta = path.metadata().unwrap();
        let i = (self.i.len()) as u64;

        let kind = match meta.is_dir() {
            true  => 1,
            false => 2,
        };

        let entry = Inode{
            i:  i,
            p: parent_inode,
            s: meta.len(),
            k: kind,
            a: 0o775,

            d: None,
            h: None,
            c: Some(Vec::new()),

            host_path: path.path().into_os_string(),
        };

        self.i.push(entry);

        (
            path.file_name().to_string_lossy().into_owned(),
            ContentDirEntry {
                i: i,
                k: kind,
            },
        )
    }

    fn descend(&mut self, parent_inode: u64, path: ::std::ffi::OsString) {

        let dirs = collect_dir(path).unwrap();

        let inode_start = self.i.len() as u64;
        let inode_len   = dirs.len() as u64;

        // 1 iteration to create all the inodes
        let mut contentdirmap : HashMap<String, ContentDirEntry> = HashMap::new();
        for path in dirs {
            let (name, cde) = self.add_from_dir_entry(parent_inode, path);
            contentdirmap.insert(name, cde);
        }

        // insert the dirmap into the current parent node
        self.i[parent_inode as usize].d = Some(contentdirmap);

        // 2. iteration to descend into the subdirs
        for x in inode_start..(inode_start+inode_len) {
            let (kind, inode, path) = {
                let ref e = self.i[x as usize];
                (e.k, e.i, e.host_path.clone())
            };
            if kind == 1 {
                self.descend(inode, path);
            }
        }
    }
}

pub fn from_host(host: ::std::ffi::OsString) -> Index{
    let mut index = Index{
        v: 1,
        i: Vec::new(),
        c: None,
    };

    index.i.push(Inode{
        i: 0,
        p: 0,
        s: 0,
        k: 1,
        a: 0o775,

        d: None,
        h: None,
        c: None,

        host_path: host.clone(),
    });
    index.descend(0, host);
    index
}

