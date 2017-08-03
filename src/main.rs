extern crate fuse;
extern crate time;
extern crate libc;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rmp_serde as rmps;
extern crate sha2;
extern crate digest;
extern crate rollsum;
extern crate pbr;
extern crate clap;
extern crate url;
extern crate generic_array;
extern crate hex;

mod fs;
mod serializer;
mod index;
mod blockstore;
mod readchain;
mod localstore;

use std::ffi::OsString;
use clap::{Arg, App, SubCommand, AppSettings};
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::Path;
use url::{Url, ParseError};
use hex::ToHex;

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

            let mut bs = blockstore::new();
            let bsp = store_path.join("content");
            if !bsp.exists() {
                println!("{:?} doesn't look like a content store. maybe you want to run store init?", target_path);
                std::process::exit(10);
            }
            bs.load(&bsp);
            let mut hi = index::from_host(OsString::from(root_path));
            hi.serialize_to_blocks(&mut bs);

            for (hs, block) in &bs.blocks {
                let hs = hs.to_hex();
                let mut p = bsp.join(&hs[0..2]);
                create_dir_all(&p).unwrap();
                p = p.join(&hs[2..]);
                if p.exists() {
                    //TODO double check shasum
                } else {
                    let mut f = File::create(&p).unwrap();
                    std::io::copy(&mut block.chain(), &mut f);
                    f.flush();
                }
            }

            let mut local = localstore::new(store_path);
            local.store(&hi, "test", &mut bs);
        },
        _ => unreachable!()
    }



    //let j   = serde_json::to_string(&hi).unwrap();
    //println!("{}", j);


    return;

    //let fs = fs::Fuse::new(&hi, &bs);

    //let mountpoint  = env::args_os().nth(2).unwrap();
    //let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];
    //fuse::mount(fs, &mountpoint, &fuse_args).unwrap();
}


#[test]
fn snail() {
    let mut bs = blockstore::new();
    let mut hi = index::from_host(std::ffi::OsString::from("."));
    hi.serialize(&mut bs);

}
