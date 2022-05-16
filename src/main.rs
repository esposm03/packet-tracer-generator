use std::{collections::HashMap, io::{ErrorKind, Write}, fs::File};

use packet_tracer_generator::{App, Redistributions};

use linked_hash_map::LinkedHashMap;
use serde::Deserialize;

fn main() {
    let commands = std::fs::read_to_string("commands.yml").expect("Error opening file `commands.yml`");

    let mut app = App::new();
    let mut keys = HashMap::new();
    let document = serde_yaml::from_str::<Document>(&commands).expect("`commands.yml` is not valid YAML");

    for (ref name, device) in &document.devices {
        keys.insert(
            name.to_string(),
            app.add_device(name)
                .position(device.x, device.y)
                .redistribute_ospf_to_rip(device.redistributions.ospf_to_rip)
                .finish(),
        );
    }

    for link in document.links {
        let r1 = link.r1.as_str();
        let r2 = link.r2.as_str();
        app.link(keys[r1], keys[r2], &link.ip, link.ospf);
    }

    match std::fs::create_dir("output").map_err(|e| e.kind()) {
        Ok(()) | Err(ErrorKind::AlreadyExists) => {}
        Err(e) => panic!("Cannot create dir `output`: {:?}", e), 
    }

    for (dev_name, commands) in app.to_commands() {
        let filename = format!("output/{dev_name}.txt");

        let mut file = File::create(&filename).expect(&format!("Failed to create file {filename}"));
        file.write_all(commands.as_bytes()).expect(&format!("Failed to write to file {filename}"));
        drop(file);

        println!("Written file `{filename}`");
    }
}

#[derive(Debug, Deserialize)]
struct Document {
    devices: LinkedHashMap<String, Router>,
    links: Vec<Link>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Router {
    redistributions: Redistributions,
    x: f32,
    y: f32,
}

#[derive(Debug, Deserialize)]
struct Link {
    r1: String,
    r2: String,
    ospf: Option<u16>,
    ip: String,
}
