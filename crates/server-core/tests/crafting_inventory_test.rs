//! Parity tests for inventory + crafting + anvil ports.
//! Covers the checklist from the crafting task: 2x2 and 3x3 crafts, Q drop,
//! hotbar moves, creative creates and anvil XP costs.

use bedrock_protocol::packets::item_stack_request::{
    ItemStackContainerSlot, ItemStackRequest, ItemStackRequestAction, ItemStackRequestPacket,
};
use bedrock_protocol::ItemStack;
use server_core::anvil::{
    anvil_combine, apply_repair, max_durability, AnvilState, MAX_REPAIR_LEVEL,
};
use server_core::crafting::{
    apply_item_stack_request, container_slot, CraftingGrid, CraftingTableInventory, manager,
    PlayerCraftingInventory, CREATED_ITEM_SLOT, UI_ARMOR, UI_COMBINED_HOTBAR_AND_INVENTORY,
    UI_CREATED_OUTPUT, UI_CURSOR, UI_INVENTORY,
};
use server_core::inventory::{
    item_network_id, PlayerInventory, ArmorInventory, WINDOW_ID_INVENTORY,
};
use server_core::PlayerSession;
use std::net::SocketAddr;

fn stack(name: &str, count: u16) -> ItemStack {
    ItemStack::new(item_network_id(&format!("minecraft:{name}")).unwrap(), count)
}

fn slot(container: u8, slot: u8) -> ItemStackContainerSlot {
    ItemStackContainerSlot::new(container, slot)
}

/// 2x2 craft through the shared manager: oak log -> 4 oak planks.
#[test]
fn craft_2x2_log_into_planks() {
    let mgr = manager();
    let mut grid = CraftingGrid::small();
    grid.slots[0] = stack("oak_log", 1);
    let outcome = mgr.craft_from_grid(&grid).expect("log -> planks");
    assert_eq!(outcome.result.network_id, item_network_id("minecraft:oak_planks").unwrap());
    assert_eq!(outcome.result.count, 4);

    // The player crafting inventory wraps the same 2x2 grid.
    let mut player_grid = PlayerCraftingInventory::new();
    player_grid.inventory.set_input(0, stack("oak_log", 1));
    player_grid.update_result();
    assert_eq!(player_grid.inventory.result.network_id, outcome.result.network_id);
    let taken = player_grid.take_result().expect("result ready");
    assert_eq!(taken.count, 4);
    assert!(player_grid.inventory.grid.is_empty());
}

/// 3x3 craft through the crafting-table inventory: planks -> crafting table.
#[test]
fn craft_3x3_table() {
    let mut table = CraftingTableInventory::new();
    table.inventory.set_input(6, stack("oak_planks", 1));
    table.inventory.set_input(7, stack("oak_planks", 1));
    table.inventory.set_input(3, stack("oak_planks", 1));
    table.inventory.set_input(4, stack("oak_planks", 1));
    table.update_result();
    assert_eq!(
        table.inventory.result.network_id,
        item_network_id("minecraft:crafting_table").unwrap()
    );
    let taken = table.take_result().expect("result ready");
    assert_eq!(taken.network_id, item_network_id("minecraft:crafting_table").unwrap());
    assert!(table.inventory.grid.is_empty());
}

/// Q drop: dropping halves a hotbar stack and reports the dropped stack.
#[test]
fn drop_with_q_key() {
    let mut inventory = PlayerInventory::new();
    inventory.set_window_slot(WINDOW_ID_INVENTORY, 4, stack("stone", 64));
    let request = ItemStackRequest::new(
        10,
        vec![ItemStackRequestAction::Drop {
            count: 32,
            source: slot(UI_COMBINED_HOTBAR_AND_INVENTORY, 4),
            randomly: false,
        }],
    );
    let outcome = apply_item_stack_request(&mut inventory, manager(), &request, false, &[]);
    assert_eq!(outcome.response.status, server_core::crafting::STATUS_OK);
    assert_eq!(outcome.dropped, vec![stack("stone", 32)]);
    assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 4).unwrap().count, 32);
}

/// Hotbar move: moving a stack between hotbar slots keeps its contents.
#[test]
fn move_hotbar_stack() {
    let mut inventory = PlayerInventory::new();
    inventory.set_window_slot(WINDOW_ID_INVENTORY, 2, stack("diamond_sword", 1));
    inventory.swap_inventory_slots(2, 6);
    assert!(inventory.window_slot(WINDOW_ID_INVENTORY, 2).unwrap().is_empty());
    assert_eq!(
        inventory.window_slot(WINDOW_ID_INVENTORY, 6).unwrap().network_id,
        stack("diamond_sword", 1).network_id
    );
    assert_eq!(inventory.held_item(), ItemStack::empty());

    // Number-key equivalent: select the hotbar slot and reveal the held item.
    inventory.set_selected_hotbar_slot(6);
    assert_eq!(inventory.held_item().network_id, stack("diamond_sword", 1).network_id);
}

/// Creative creation: a creative player drags a full stack from the creative
/// catalog into hotbar slot 0 (CreativeCreate fills the created-item output,
/// then the client takes it).
#[test]
fn creative_create_item() {
    let mut inventory = PlayerInventory::new();
    let catalog = vec![stack("stone", 64)];
    let request = ItemStackRequest::new(
        11,
        vec![
            ItemStackRequestAction::CreativeCreate {
                creative_item_id: 1,
                repetitions: 64,
            },
            ItemStackRequestAction::Take {
                count: 64,
                source: slot(UI_CREATED_OUTPUT, CREATED_ITEM_SLOT),
                destination: slot(UI_COMBINED_HOTBAR_AND_INVENTORY, 0),
            },
        ],
    );
    let outcome = apply_item_stack_request(&mut inventory, manager(), &request, true, &catalog);
    assert_eq!(outcome.response.status, server_core::crafting::STATUS_OK);
    assert_eq!(inventory.total_items(), 64);
    assert_eq!(inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().network_id, stack("stone", 1).network_id);
}

/// Survival players cannot creative-create: the request is rejected and the
/// inventory is left untouched.
#[test]
fn survival_creative_create_is_rejected() {
    let mut inventory = PlayerInventory::new();
    let catalog = vec![stack("stone", 64)];
    let request = ItemStackRequest::new(
        12,
        vec![ItemStackRequestAction::CreativeCreate {
            creative_item_id: 1,
            repetitions: 64,
        }],
    );
    let outcome = apply_item_stack_request(&mut inventory, manager(), &request, false, &catalog);
    assert_eq!(outcome.response.status, server_core::crafting::STATUS_ERROR);
    assert!(inventory.is_empty());
}

/// Anvil repair cost: two damaged diamond swords combine into one repaired
/// sword, deducting XP levels.
#[test]
fn anvil_repair_cost_and_xp() {
    let sword = item_network_id("minecraft:diamond_sword").unwrap();
    assert_eq!(max_durability(sword), 1561);
    let mut left = ItemStack::new(sword, 1);
    let right = ItemStack::new(sword, 1);
    left.meta = 780; // half damaged
    let repair = anvil_combine(&left, &right).expect("repairable");
    assert!(repair.cost >= 1 && repair.cost <= MAX_REPAIR_LEVEL);

    let mut inventory = PlayerInventory::new();
    inventory.set_window_slot(WINDOW_ID_INVENTORY, 0, left.clone());
    inventory.set_window_slot(WINDOW_ID_INVENTORY, 1, right.clone());
    let mut xp = 10;
    let leftover = apply_repair(&mut inventory, 0, 1, &repair, &mut xp);
    assert_eq!(leftover, Some(0));
    assert_eq!(xp, 10 - repair.cost);
    assert_eq!(inventory.total_items(), 1);
    let result = inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().clone();
    assert_eq!(result.network_id, sword);
    assert!(result.meta < 780, "result should be less damaged");
}

/// Anvil state machine: repeated hits progress damage and eventually break.
#[test]
fn anvil_damage_progression() {
    let mut anvil = AnvilState::new(0);
    assert_eq!(anvil.damage(90).damage, 1);
    assert_eq!(anvil.damage(90).damage, 2);
    assert_eq!(anvil.damage(90).damage, 3);
    assert!(anvil.damage(90).broken);
}

/// Armor, offhand and cursor plus cursor moves through ItemStackRequest.
#[test]
fn armor_and_cursor_through_requests() {
    let mut inventory = PlayerInventory::new();
    inventory.set_window_slot(WINDOW_ID_INVENTORY, 1, stack("iron_chestplate", 1));
    // Take the chestplate to the cursor, then place it into armor slot 1.
    let request = ItemStackRequest::new(
        13,
        vec![
            ItemStackRequestAction::Take {
                count: 1,
                source: slot(UI_INVENTORY, 1),
                destination: slot(UI_CURSOR, 0),
            },
            ItemStackRequestAction::Take {
                count: 1,
                source: slot(UI_CURSOR, 0),
                destination: slot(UI_ARMOR, 1),
            },
        ],
    );
    let outcome = apply_item_stack_request(&mut inventory, manager(), &request, false, &[]);
    assert_eq!(outcome.response.status, server_core::crafting::STATUS_OK);
    assert_eq!(
        inventory.window_slot(server_core::inventory::WINDOW_ID_ARMOR, 1).unwrap().network_id,
        stack("iron_chestplate", 1).network_id
    );
    assert!(inventory.cursor_item().is_empty());
    // `container_slot` agrees with the window view.
    assert_eq!(
        container_slot(&inventory, &slot(UI_CURSOR, 0)),
        Some(ItemStack::empty())
    );
    let _ = ArmorInventory::new();
}

/// Session-level: ItemStackRequest answered over the wire with 0x94.
#[test]
fn session_answers_item_stack_request() {
    let mut session = PlayerSession::new(7, SocketAddr::from(([127, 0, 0, 1], 19132)));
    session
        .inventory
        .set_window_slot(WINDOW_ID_INVENTORY, 0, stack("stone", 64));
    let mut writer = bedrock_protocol::binary::Writer::new();
    ItemStackRequestPacket::new(
        21,
        vec![ItemStackRequestAction::Drop {
            count: 5,
            source: slot(UI_COMBINED_HOTBAR_AND_INVENTORY, 0),
            randomly: false,
        }],
    )
    .encode_payload(&mut writer);
    let out = session.handle_packet(0x93, &writer.into_bytes()).unwrap();
    assert_eq!(out.len(), 3); // response + slot update + mob equipment
    assert_eq!(out[0].packet_id(), 0x94);
    assert_eq!(out[2].packet_id(), 0x1F);
    assert_eq!(session.inventory.window_slot(WINDOW_ID_INVENTORY, 0).unwrap().count, 59);
}

/// Session-level: the creative drag that failed before (the client sends
/// `CreativeCreate` for the catalog entry, then takes it from the created-item
/// output) now reaches the hotbar and is answered with an OK response.
#[test]
fn session_creative_drag_reaches_the_hotbar() {
    let mut session = PlayerSession::new(11, SocketAddr::from(([127, 0, 0, 1], 19132)));
    session.gamemode = 1;
    let mut writer = bedrock_protocol::binary::Writer::new();
    ItemStackRequestPacket::new(
        30,
        vec![
            ItemStackRequestAction::CreativeCreate {
                creative_item_id: 1,
                repetitions: 1,
            },
            ItemStackRequestAction::Take {
                count: 1,
                source: slot(UI_CREATED_OUTPUT, CREATED_ITEM_SLOT),
                destination: slot(UI_COMBINED_HOTBAR_AND_INVENTORY, 0),
            },
        ],
    )
    .encode_payload(&mut writer);
    let out = session.handle_packet(0x93, &writer.into_bytes()).unwrap();
    assert_eq!(out[0].packet_id(), 0x94);
    let catalog = server_core::crafting::creative_catalog();
    assert!(!catalog[0].is_empty(), "catalog entry 1 must be a real item");
    assert_eq!(
        session
            .inventory
            .window_slot(WINDOW_ID_INVENTORY, 0)
            .unwrap()
            .network_id,
        catalog[0].network_id
    );
}

/// Session-level: opening and closing the crafting table container.
#[test]
fn session_opens_and_closes_crafting_table() {
    let mut session = PlayerSession::new(8, SocketAddr::from(([127, 0, 0, 1], 19132)));
    let out = session.open_crafting_table((10, 64, 10));
    assert_eq!(out[0].packet_id(), 0x2E);
    assert!(session.open_container.is_some());
    out[0].packet_id(); // 0x2E sent

    // Put planks in the table grid, then close: inputs return to inventory.
    session.crafting_table.inventory.set_input(0, stack("oak_planks", 4));
    let out = session.handle_packet(0x2F, b"\x0a\x01\x00").unwrap();
    assert!(out.iter().any(|packet| packet.packet_id() == 0x2F));
    assert!(session.open_container.is_none());
    assert_eq!(session.inventory.total_items(), 4);
}

/// Session-level: InteractPacket OPEN_INVENTORY (0x21) opens the inventory
/// screen (ContainerOpen 0x2E, window 0, container type `INVENTORY`) and then
/// resyncs the contents.
#[test]
fn session_interact_open_inventory() {
    use bedrock_protocol::packets::container_open::{
        ContainerOpenPacket, CONTAINER_TYPE_INVENTORY,
    };
    use bedrock_protocol::packets::interact::{InteractPacket, ACTION_OPEN_INVENTORY};
    let mut session = PlayerSession::new(9, SocketAddr::from(([127, 0, 0, 1], 19132)));
    let mut writer = bedrock_protocol::binary::Writer::new();
    InteractPacket::new(ACTION_OPEN_INVENTORY, 9, None).encode_payload(&mut writer);
    let out = session.handle_packet(0x21, &writer.into_bytes()).unwrap();
    assert_eq!(out[0].packet_id(), 0x2E); // ContainerOpenPacket
    assert_eq!(out[1].packet_id(), 0x31); // InventoryContentPacket

    let bytes = out[0].to_bytes().unwrap();
    let mut reader = bedrock_protocol::binary::Reader::new(&bytes);
    assert_eq!(reader.read_var_u32().unwrap(), ContainerOpenPacket::PACKET_ID);
    let open = ContainerOpenPacket::decode_payload(&mut reader).unwrap();
    assert_eq!(
        open.window_id as u32,
        server_core::inventory::WINDOW_ID_INVENTORY
    );
    assert_eq!(open.container_type, CONTAINER_TYPE_INVENTORY);
    assert_eq!(open.actor_unique_id, session.entity_runtime_id as i64);
}

/// Session-level: anvil container + damage roll through its API.
#[test]
fn session_anvil_open_and_damage() {
    let mut session = PlayerSession::new(10, SocketAddr::from(([127, 0, 0, 1], 19132)));
    let out = session.open_anvil((5, 64, 5));
    assert_eq!(out[0].packet_id(), 0x2E);
    assert_eq!(session.open_container.as_ref().unwrap().window_id, 16);
    // Roll 90 never breaks early stages.
    assert!(!session.damage_anvil_at(5, 64, 5, 90));
    assert_eq!(session.anvil_damage, 1);
}

/// Session spawn bundles carry the recipe book and the creative catalog.
#[test]
fn spawn_wires_include_crafting_and_creative() {
    let crafting = PlayerSession::crafting_data_wire();
    let creative = PlayerSession::creative_content_wire();
    let crafting_id = bedrock_protocol::binary::Reader::new(&crafting).read_var_u32().unwrap();
    let creative_id = bedrock_protocol::binary::Reader::new(&creative).read_var_u32().unwrap();
    assert_eq!(crafting_id, 0x34);
    assert_eq!(creative_id, 0x91);
    assert!(crafting.len() > 10_000);
    assert!(creative.len() > 10_000);
}
