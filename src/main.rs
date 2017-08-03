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
use std::ffi::OsString;
use std::fs::{create_dir_all};
use std::path::Path;
use url::{Url};
use std::ffi::OsStr;

fn main() {

    let matches = App::new("korhal-image")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpSubcommand)
        .version("1.0")
        .about("content addressable image indexer")
        .subcommand(
            SubCommand::with_name("pack")
            .about("write image into .tcxz")
            .arg(Arg::with_name("root")
                 .required(true)
                 .help("build image from this path")
                 .takes_value(true)
                 .index(1)
                )
            .arg(Arg::with_name("target")
                 .help("file path to write .tcxz")
                 .default_value("out.tcxz")
                 .takes_value(true)
                 .index(2)
                )
            )
        .subcommand(
            SubCommand::with_name("push")
            .about("write image into content store")
            .arg(Arg::with_name("root")
                 .required(true)
                 .help("build image from this path")
                 .takes_value(true)
                 .index(1)
                )
            .arg(Arg::with_name("target")
                 .required(true)
                 .help("url to content store and index name in the form scheme://path/index-name")
                 .takes_value(true)
                 .index(2)
                )
            )
        .subcommand(
            SubCommand::with_name("store-init")
            .about("initialize a store")
            .arg(Arg::with_name("target")
                 .required(true)
                 .help("path to init as store")
                 .takes_value(true)
                 .index(1)
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



    match matches.subcommand() {
        ("store-init", Some(submatches)) =>{
            let target_url = Url::parse(submatches.value_of("target").unwrap()).unwrap();

            match target_url.scheme() {
                "" | "file" => {},
                _ => panic!(format!("{} is not a supported store scheme", target_url.scheme())),
            }
            let target_path = Path::new(target_url.path());

            create_dir_all(&target_path.join("content")).unwrap();
        },
        ("push", Some(submatches)) =>{
            let root_path  = submatches.value_of("root").unwrap();
            let target_url = Url::parse(submatches.value_of("target").unwrap()).unwrap();

            match target_url.scheme() {
                "" | "file" => {},
                _ => panic!(format!("{} is not a supported store scheme", target_url.scheme())),
            }

            let target_path = Path::new(target_url.path());
            let store_path = target_path.parent().unwrap();

            let bsp = store_path.join("content");
            if !bsp.exists() {
                println!("{:?} doesn't look like a content store. maybe you want to run store init?", target_path);
                std::process::exit(10);
            }
            let mut bs = blockstore::new(bsp.to_str().unwrap().to_owned());

            let mut hi = index::from_host(OsString::from(root_path));
            hi.store_inodes(&mut bs);

            loop {
                hi = hi.store_index(&mut bs);
                if hi.c.as_ref().unwrap().len() == 1 {
                    break;
                }
            }

            hi.save_to_file(target_path);
            println!("input stored into index {} with name {:?}",
                     hi.c.as_ref().unwrap().first().unwrap().h.to_hex(),
                     target_path.file_name().unwrap()
                     )
        },
        ("mount", Some(submatches)) =>{
            let source_url  = Url::parse(submatches.value_of("source").unwrap()).unwrap();
            let target_path = submatches.value_of("target").unwrap();


            let source_path = Path::new(source_url.path());
            let store_path = source_path.parent().unwrap();

            let bsp = store_path.join("content");
            if !bsp.exists() {
                println!("{:?} doesn't look like a content store. maybe you want to run store init?", target_path);
                std::process::exit(10);
            }
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
