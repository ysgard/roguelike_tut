mod item_structs;
mod rawmaster;
use item_structs::*;
pub use rawmaster::*;
use std::sync::Mutex;

#[derive(Deserialize, Debug)]
pub struct Raws {
    pub items: Vec<Item>,
    pub mobs: Vec<Mob>
}

lazy_static! {
    pub static ref RAWS: Mutex<RawMaster> = Mutex::new(RawMaster::empty());
}

rltk::embedded_resource!(RAW_FILE, "../../raws/spawns.json");

pub fn load_raws() {
    rltk::link_resource!(RAW_FILE, "../../raws/spawns.json");

    // Retrieve the raw data as an arraay of u8
    let raw_data = rltk::embedding::EMBED
        .lock()
        .unwrap()
        .get_resource("../../raws/spawns.json".to_string())
        .unwrap();
    let raw_string =
        std::str::from_utf8(&raw_data).expect("Unable to convert to a valid UTF-8 string.");
    let decoder: Raws = serde_json::from_str(&raw_string).expect("Unable to parse JSON");

    RAWS.lock().unwrap().load(decoder);
}
