use super::{spawner, Map, Position, Rect, TileType, SHOW_MAPGEN_VISUALIZER};
mod common;
use common::*;
mod area_starting_point;
mod bsp_dungeon;
mod bsp_interior;
mod cellular_automata;
mod cull_unreachable;
mod distant_exit;
mod dla;
mod drunkard;
mod maze;
mod prefab_builder;
mod room_based_spawner;
mod room_based_stairs;
mod room_based_starting_position;
mod simple_map;
mod voronoi;
mod voronoi_spawning;
mod waveform_collapse;
use area_starting_point::{AreaStartingPosition, XStart, YStart};
use bsp_dungeon::BspDungeonBuilder;
use bsp_interior::BspInteriorBuilder;
use cellular_automata::CellularAutomataBuilder;
use cull_unreachable::CullUnreachable;
use distant_exit::DistantExit;
use dla::DLABuilder;
use drunkard::DrunkardsWalkBuilder;
use maze::MazeBuilder;
use prefab_builder::PrefabBuilder;
use room_based_spawner::RoomBasedSpawner;
use room_based_stairs::RoomBasedStairs;
use room_based_starting_position::RoomBasedStartingPosition;
use simple_map::SimpleMapBuilder;
use voronoi::VoronoiCellBuilder;
use voronoi_spawning::VoronoiSpawning;
use waveform_collapse::WaveformCollapseBuilder;

use specs::prelude::*;

pub trait InitialMapBuilder {
    fn build_map(&mut self, rng: &mut rltk::RandomNumberGenerator, build_data: &mut BuilderMap);
}

pub trait MetaMapBuilder {
    fn build_map(&mut self, rng: &mut rltk::RandomNumberGenerator, build_data: &mut BuilderMap);
}

pub struct BuilderMap {
    pub spawn_list: Vec<(usize, String)>,
    pub map: Map,
    pub starting_position: Option<Position>,
    pub rooms: Option<Vec<Rect>>,
    pub history: Vec<Map>,
}

impl BuilderMap {
    fn take_snapshot(&mut self) {
        if SHOW_MAPGEN_VISUALIZER {
            let mut snapshot = self.map.clone();
            for v in snapshot.revealed_tiles.iter_mut() {
                *v = true;
            }
            self.history.push(snapshot);
        }
    }
}

pub struct BuilderChain {
    starter: Option<Box<dyn InitialMapBuilder>>,
    builders: Vec<Box<dyn MetaMapBuilder>>,
    pub build_data: BuilderMap,
}

impl BuilderChain {
    pub fn new(new_depth: i32) -> BuilderChain {
        BuilderChain {
            starter: None,
            builders: Vec::new(),
            build_data: BuilderMap {
                spawn_list: Vec::new(),
                map: Map::new(new_depth),
                starting_position: None,
                rooms: None,
                history: Vec::new(),
            },
        }
    }

    pub fn start_with(&mut self, starter: Box<dyn InitialMapBuilder>) {
        match self.starter {
            None => self.starter = Some(starter),
            Some(_) => panic!("You can only have one starting builder."),
        };
    }

    pub fn with(&mut self, metabuilder: Box<dyn MetaMapBuilder>) {
        self.builders.push(metabuilder);
    }

    pub fn build_map(&mut self, rng: &mut rltk::RandomNumberGenerator) {
        match &mut self.starter {
            None => panic!("Cannot run a map builder chain without a starting build system"),
            Some(starter) => {
                // Build the starting map
                starter.build_map(rng, &mut self.build_data);
            }
        }

        // Build additional layers in turn
        for metabuilder in self.builders.iter_mut() {
            metabuilder.build_map(rng, &mut self.build_data);
        }
    }

    pub fn spawn_entities(&mut self, ecs: &mut World) {
        for entity in self.build_data.spawn_list.iter() {
            spawner::spawn_entity(ecs, &(&entity.0, &entity.1));
        }
    }
}

fn random_initial_builder(
    rng: &mut rltk::RandomNumberGenerator,
) -> (Box<dyn InitialMapBuilder>, bool) {
    let builder = rng.roll_dice(1, 17);
    let result: (Box<dyn InitialMapBuilder>, bool);
    match builder {
        1 => result = (BspDungeonBuilder::new(), true),
        2 => result = (BspInteriorBuilder::new(), true),
        3 => result = (CellularAutomataBuilder::new(), false),
        4 => result = (DrunkardsWalkBuilder::open_area(), false),
        5 => result = (DrunkardsWalkBuilder::open_halls(), false),
        6 => result = (DrunkardsWalkBuilder::winding_passages(), false),
        7 => result = (DrunkardsWalkBuilder::fat_passages(), false),
        8 => result = (DrunkardsWalkBuilder::fearful_symmetry(), false),
        9 => result = (MazeBuilder::new(), false),
        10 => result = (DLABuilder::walk_inwards(), false),
        11 => result = (DLABuilder::walk_outwards(), false),
        12 => result = (DLABuilder::central_attractor(), false),
        13 => result = (DLABuilder::insectoid(), false),
        14 => result = (VoronoiCellBuilder::pythagoras(), false),
        15 => result = (VoronoiCellBuilder::manhattan(), false),
        16 => {
            result = (
                PrefabBuilder::constant(prefab_builder::prefab_levels::WFC_POPULATED),
                false,
            )
        }
        _ => result = (SimpleMapBuilder::new(), true),
    }
    result
}

pub fn random_builder(new_depth: i32, rng: &mut rltk::RandomNumberGenerator) -> BuilderChain {
    let mut builder = BuilderChain::new(new_depth);
    let (random_starter, has_rooms) = random_initial_builder(rng);
    builder.start_with(random_starter);
    if has_rooms {
        builder.with(RoomBasedSpawner::new());
        builder.with(RoomBasedStairs::new());
        builder.with(RoomBasedStartingPosition::new());
    } else {
        builder.with(AreaStartingPosition::new(XStart::CENTER, YStart::CENTER));
        builder.with(CullUnreachable::new());
        builder.with(VoronoiSpawning::new());
        builder.with(DistantExit::new());
    }

    if rng.roll_dice(1, 3) == 1 {
        builder.with(WaveformCollapseBuilder::new());
    }

    if rng.roll_dice(1, 20) == 1 {
        builder.with(PrefabBuilder::sectional(
            prefab_builder::prefab_sections::UNDERGROUND_FORT,
        ));
    }

    builder.with(PrefabBuilder::vaults());

    builder
}
