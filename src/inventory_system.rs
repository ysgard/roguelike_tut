extern crate specs;
use super::{
    gamelog::GameLog, particle_system::ParticleBuilder, AreaOfEffect, CombatStats, Confusion,
    Consumable, Equippable, Equipped, HungerClock, HungerState, InBackpack, InflictsDamage,
    MagicMapper, Map, Name, Position, ProvidesFood, ProvidesHealing, RunState, SufferDamage,
    WantsToDropItem, WantsToPickupItem, WantsToRemoveItem, WantsToUseItem,
};
use specs::prelude::*;

pub struct ItemCollectionSystem {}

impl<'a> System<'a> for ItemCollectionSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, GameLog>,
        WriteStorage<'a, WantsToPickupItem>,
        WriteStorage<'a, Position>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (player_entity, mut gamelog, mut wants_pickup, mut positions, names, mut backpack) =
            data;

        for pickup in wants_pickup.join() {
            positions.remove(pickup.item);
            backpack
                .insert(
                    pickup.item,
                    InBackpack {
                        owner: pickup.collected_by,
                    },
                )
                .expect("ItemCollectionSystem: Unable to insert backpack entry");

            if pickup.collected_by == *player_entity {
                gamelog.entries.insert(
                    0,
                    format!("You pick up the {}.", names.get(pickup.item).unwrap().name),
                );
            }
        }

        wants_pickup.clear();
    }
}

pub struct ItemUseSystem {}

impl<'a> System<'a> for ItemUseSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, GameLog>,
        WriteExpect<'a, Map>,
        Entities<'a>,
        WriteStorage<'a, WantsToUseItem>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, CombatStats>,
        ReadStorage<'a, Consumable>,
        ReadStorage<'a, ProvidesHealing>,
        ReadStorage<'a, InflictsDamage>,
        WriteStorage<'a, SufferDamage>,
        ReadStorage<'a, AreaOfEffect>,
        WriteStorage<'a, Confusion>,
        ReadStorage<'a, Equippable>,
        WriteStorage<'a, Equipped>,
        WriteStorage<'a, InBackpack>,
        WriteExpect<'a, ParticleBuilder>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, ProvidesFood>,
        WriteStorage<'a, HungerClock>,
        ReadStorage<'a, MagicMapper>,
        WriteExpect<'a, RunState>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            player_entity,
            mut gamelog,
            map,
            entities,
            mut wants_use,
            names,
            mut combat_stats,
            consumables,
            healing,
            inflict_damage,
            mut suffer_damage,
            aoe,
            mut confused,
            equippable,
            mut equipped,
            mut backpack,
            mut particle_builder,
            positions,
            provides_food,
            mut hunger_clocks,
            magic_mapper,
            mut runstate,
        ) = data;

        for (entity, useitem) in (&entities, &wants_use).join() {
            let mut used_item = true;

            // Targeting
            let mut targets: Vec<Entity> = Vec::new();
            match useitem.target {
                None => {
                    targets.push(*player_entity);
                }
                Some(target) => {
                    let area_effect = aoe.get(useitem.item);
                    match area_effect {
                        None => {
                            // Single target in tile
                            let idx = map.xy_idx(target.x, target.y);
                            for mob in map.tile_content[idx].iter() {
                                targets.push(*mob);
                            }
                        }
                        Some(area_effect) => {
                            // AoE
                            let mut blast_tiles =
                                rltk::field_of_view(target, area_effect.radius, &*map);
                            blast_tiles.retain(|p| {
                                p.x > 0 && p.x < map.width - 1 && p.y > 0 && p.y < map.height - 1
                            });
                            for tile_idx in blast_tiles.iter() {
                                let idx = map.xy_idx(tile_idx.x, tile_idx.y);
                                for mob in map.tile_content[idx].iter() {
                                    targets.push(*mob);
                                }
                                particle_builder.request(
                                    tile_idx.x,
                                    tile_idx.y,
                                    rltk::RGB::named(rltk::ORANGE),
                                    rltk::RGB::named(rltk::BLACK),
                                    rltk::to_cp437('░'),
                                    200.0,
                                );
                            }
                        }
                    }
                }
            }

            // If it is equippable, then we want to equip it - and unequip whatever else was in that slot
            let item_equippable = equippable.get(useitem.item);
            match item_equippable {
                None => {}
                Some(can_equip) => {
                    let target_slot = can_equip.slot;
                    let target = targets[0];

                    // Remove any items the target has in the item's slot
                    let mut to_unequip: Vec<Entity> = Vec::new();
                    for (item_entity, already_equipped, name) in
                        (&entities, &equipped, &names).join()
                    {
                        if already_equipped.owner == target && already_equipped.slot == target_slot
                        {
                            to_unequip.push(item_entity);
                            if target == *player_entity {
                                gamelog
                                    .entries
                                    .insert(0, format!("You unequip {}.", name.name));
                            }
                        }
                    }
                    for item in to_unequip.iter() {
                        equipped.remove(*item);
                        backpack
                            .insert(*item, InBackpack { owner: target })
                            .expect("ItemUseSystem: unable to insert backpack entry");
                    }

                    // Wield the item
                    equipped
                        .insert(
                            useitem.item,
                            Equipped {
                                owner: target,
                                slot: target_slot,
                            },
                        )
                        .expect("ItemUseSystem: unable to insert equipped component");
                    backpack.remove(useitem.item);
                    if target == *player_entity {
                        gamelog.entries.insert(
                            0,
                            format!("You equip {}.", names.get(useitem.item).unwrap().name),
                        );
                    }
                }
            }

            // If it is edible, eat it!
            let item_edible = provides_food.get(useitem.item);
            match item_edible {
                None => {}
                Some(_) => {
                    used_item = true;
                    let target = targets[0];
                    let hc = hunger_clocks.get_mut(target);
                    if let Some(hc) = hc {
                        hc.state = HungerState::WellFed;
                        hc.duration = 20;
                        gamelog.entries.insert(
                            0,
                            format!("You eat the {}.", names.get(useitem.item).unwrap().name),
                        );
                    }
                }
            }

            // If it heals, apply the healing
            let item_heals = healing.get(useitem.item);
            match item_heals {
                None => {}
                Some(healer) => {
                    for target in targets.iter() {
                        let stats = combat_stats.get_mut(*target);
                        if let Some(stats) = stats {
                            stats.hp = i32::max(stats.max_hp, stats.hp + healer.heal_amount);
                            if entity == *player_entity {
                                gamelog.entries.insert(
                                    0,
                                    format!(
                                        "You use the {}, healing {} hp.",
                                        names.get(useitem.item).unwrap().name,
                                        healer.heal_amount
                                    ),
                                );
                            }
                            used_item = true;

                            let pos = positions.get(*target);
                            if let Some(pos) = pos {
                                particle_builder.request(
                                    pos.x,
                                    pos.y,
                                    rltk::RGB::named(rltk::GREEN),
                                    rltk::RGB::named(rltk::BLACK),
                                    rltk::to_cp437('♥'),
                                    200.0,
                                );
                            }
                        }
                    }
                }
            }

            // If its a magic mapper...
            let is_mapper = magic_mapper.get(useitem.item);
            match is_mapper {
                None => {}
                Some(_) => {
                    used_item = true;
                    gamelog
                        .entries
                        .insert(0, "The map is revealed to you!".to_string());
                    *runstate = RunState::MagicMapReveal { row: 0 };
                }
            }

            // If it inflicts damage, apply it to the target cell
            let item_damages = inflict_damage.get(useitem.item);
            match item_damages {
                None => {}
                Some(damage) => {
                    used_item = false;
                    for mob in targets.iter() {
                        suffer_damage
                            .insert(
                                *mob,
                                SufferDamage {
                                    amount: damage.damage,
                                },
                            )
                            .expect("ItemUseSystem: unable to insert damage");
                        if entity == *player_entity {
                            let mob_name = names.get(*mob).unwrap();
                            let item_name = names.get(useitem.item).unwrap();
                            gamelog.entries.insert(
                                0,
                                format!(
                                    "You use {} on {}, inflicting {} hp.",
                                    item_name.name, mob_name.name, damage.damage
                                ),
                            );
                            let pos = positions.get(*mob);
                            if let Some(pos) = pos {
                                particle_builder.request(
                                    pos.x,
                                    pos.y,
                                    rltk::RGB::named(rltk::RED),
                                    rltk::RGB::named(rltk::BLACK),
                                    rltk::to_cp437('‼'),
                                    200.0,
                                );
                            }
                        }
                        used_item = true;
                    }
                }
            }

            // Can it pass along confusion? Note the use of scopes to escape from the borrow checker
            let mut add_confusion = Vec::new();
            {
                let causes_confusion = confused.get(useitem.item);
                match causes_confusion {
                    None => {}
                    Some(confusion) => {
                        used_item = false;
                        for mob in targets.iter() {
                            add_confusion.push((*mob, confusion.turns));
                            if entity == *player_entity {
                                let mob_name = names.get(*mob).unwrap();
                                let item_name = names.get(useitem.item).unwrap();
                                gamelog.entries.insert(
                                    0,
                                    format!(
                                        "You use {} on {}, confusing them.",
                                        item_name.name, mob_name.name
                                    ),
                                );
                                let pos = positions.get(*mob);
                                if let Some(pos) = pos {
                                    particle_builder.request(
                                        pos.x,
                                        pos.y,
                                        rltk::RGB::named(rltk::MAGENTA),
                                        rltk::RGB::named(rltk::BLACK),
                                        rltk::to_cp437('?'),
                                        200.0,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            for mob in add_confusion.iter() {
                confused
                    .insert(mob.0, Confusion { turns: mob.1 })
                    .expect("ItemUseSystem: Unable to insert confusion");
            }

            // If it's a consumable, delete on use
            if used_item {
                let consumable = consumables.get(useitem.item);
                match consumable {
                    None => {}
                    Some(_) => {
                        entities
                            .delete(useitem.item)
                            .expect("ItemUseSystem: Delete failed");
                    }
                }
            }
        }
        wants_use.clear();
    }
}

pub struct ItemDropSystem {}
impl<'a> System<'a> for ItemDropSystem {
    type SystemData = (
        ReadExpect<'a, Entity>,
        WriteExpect<'a, GameLog>,
        Entities<'a>,
        WriteStorage<'a, WantsToDropItem>,
        ReadStorage<'a, Name>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (
            player_entity,
            mut gamelog,
            entities,
            mut wants_drop,
            names,
            mut positions,
            mut backpack,
        ) = data;

        for (entity, to_drop) in (&entities, &wants_drop).join() {
            let mut dropper_pos: Position = Position { x: 0, y: 0 };
            {
                let dropped_pos = positions.get(entity).unwrap();
                dropper_pos.x = dropped_pos.x;
                dropper_pos.y = dropped_pos.y;
            }
            positions
                .insert(
                    to_drop.item,
                    Position {
                        x: dropper_pos.x,
                        y: dropper_pos.y,
                    },
                )
                .expect("ItemDropSystem: Unable to insert position");
            backpack.remove(to_drop.item);

            if entity == *player_entity {
                gamelog.entries.insert(
                    0,
                    format!("You drop the {}.", names.get(to_drop.item).unwrap().name),
                );
            }
        }
        wants_drop.clear();
    }
}

pub struct ItemRemoveSystem {}

impl<'a> System<'a> for ItemRemoveSystem {
    type SystemData = (
        Entities<'a>,
        WriteStorage<'a, WantsToRemoveItem>,
        WriteStorage<'a, Equipped>,
        WriteStorage<'a, InBackpack>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (entities, mut wants_remove, mut equipped, mut backpack) = data;

        for (entity, to_remove) in (&entities, &wants_remove).join() {
            equipped.remove(to_remove.item);
            backpack
                .insert(to_remove.item, InBackpack { owner: entity })
                .expect("ItemRemoveSystem: Unable to insert to backpack");
        }

        wants_remove.clear();
    }
}
