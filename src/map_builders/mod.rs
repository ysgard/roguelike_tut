use super::{spawner, Map, Position, Rect, TileType, SHOW_MAPGEN_VISUALIZER};
mod common;
use common::*;
mod simple_map;
use simple_map::SimpleMapBuilder;
use specs::prelude::*;

pub trait MapBuilder {
    fn build_map(&mut self);
    fn spawn_entities(&mut self, ecs: &mut World);
    fn get_map(&self) -> Map;
    fn get_starting_position(&self) -> Position;
    fn get_snapshot_history(&self) -> Vec<Map>;
    fn take_snapshot(&mut self);
}

pub fn random_builder(new_depth: i32) -> Box<dyn MapBuilder> {
    // Note that until we have a second map type, this isn't even slightly random
    Box::new(SimpleMapBuilder::new(new_depth))
}
