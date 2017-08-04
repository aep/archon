extern crate clap;
extern crate digest;
extern crate fuse;
extern crate generic_array;
extern crate hex;
extern crate libc;
extern crate pbr;
extern crate rmp_serde as rmps;
extern crate rollsum;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate sha2;
extern crate tempfile;
extern crate time;
extern crate url;

mod blockstore;
mod chunker;
mod fs;
mod index;
mod readchain;
mod serializer;

use clap::{Arg, App, SubCommand, AppSettings};
use hex::ToHex;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::{create_dir_all};
use std::path::Path;
use url::{Url};

fn main() {

    let matches = App::new("korhal-image")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpSubcommand)
        .version("1.0")
        .about("content addressable image indexer")
        .subcommand(
            SubCommand::with_name("rm")
            .about("remove index from store")
            .arg(Arg::with_name("name")
                 .required(true)
                 .help("name of index")
                 .takes_value(true)
                 .index(1)
                )
            )
        .subcommand(
            SubCommand::with_name("store")
            .about("write image into content store")
            .arg(Arg::with_name("root")
                 .required(true)
                 .help("build image from this path")
                 .takes_value(true)
                 .index(1)
                )
            .arg(Arg::with_name("name")
                 .required(true)
                 .help("name of index")
                 .takes_value(true)
                 .index(2)
                )
            )
        .subcommand(
            SubCommand::with_name("mount")
            .about("fuse mount image at a given destination")
            .arg(Arg::with_name("source")
                 .required(true)
                 .help("url to content store and index name in the form scheme://path/index-name")
                 .takes_value(true)
                 .index(1)
                )
            .arg(Arg::with_name("target")
                 .required(true)
                 .help("path where to mount image")
                 .takes_value(true)
                 .index(2)
                )
            )
        .get_matches();


    let key = "KORHAL_STORE";
    let content_store_path = match env::var(key) {
        Ok(val) => {
            println!("{}: {:?}", key, val);
            val
        },
        Err(e) => {
            println!("{}: {}", key, e);
            ::std::process::exit(1);
        },
    };

    match matches.subcommand() {
        ("store", Some(submatches)) =>{

            let root_path  = submatches.value_of("root").unwrap();
            let name       = submatches.value_of("name").unwrap();
            let store_path = Path::new(&content_store_path);
            let bsp = store_path.join("content");
            create_dir_all(&bsp);
            let mut bs = blockstore::new(bsp.to_str().unwrap().to_owned());

            let mut hi = index::from_host(OsString::from(root_path));
            hi.store_inodes(&mut bs);

            loop {
                hi = hi.store_index(&mut bs);
                if hi.c.as_ref().unwrap().len() == 1 {
                    break;
                }
            }

            hi.save_to_file(&store_path.join(name));
            println!("input stored into index {} with name {:?}",
                     hi.c.as_ref().unwrap().first().unwrap().h.to_hex(),
                     name
                     )
        },
        ("mount", Some(submatches)) =>{
            let source_url  = Url::parse(submatches.value_of("source").unwrap()).unwrap();
            let target_path = submatches.value_of("target").unwrap();

            let source_path = Path::new(source_url.path());
            let store_path = source_path.parent().unwrap();

            let bsp = store_path.join("content");
            create_dir_all(&bsp);
            let mut hi = index::Index::load_from_file(source_path);
            let bs = blockstore::new(bsp.to_str().unwrap().to_owned());
            while let Some(_) = hi.c.as_ref() {
                hi = hi.load_index(&bs);
            }

            println!("mounting index {:?} with {} inodes to {}", source_path.file_name().unwrap(), hi.i.len(), target_path);

            let fs = fs::Fuse::new(&hi, &bs);
            let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];
            fuse::mount(fs, &target_path, &fuse_args).unwrap();
        }
        ("rm", Some(submatches)) =>{
            let name = submatches.value_of("name").unwrap();

        },
        _ => unreachable!()
    }



    //let j   = serde_json::to_string(&hi).unwrap();
    //println!("{}", j);


    return;


}


#[test]
fn snail() {
    let mut bs = blockstore::new();
    let mut hi = index::from_host(std::ffi::OsString::from("."));
    hi.serialize(&mut bs);

}
