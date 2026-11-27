//! Slayer's source move visuals are driven by the physical effect actor.
//! Override the corresponding stock ACMD effects without calling One-Slot Effects.
use smash::lua2cpp::L2CAgentBase;
use smashline::{Agent, Priority};

unsafe extern "C" fn empty(_: &mut L2CAgentBase) {}

pub fn install() {
    let mut agent = Agent::new("captain");
    agent.set_costume(crate::slots());
    for script in [
        "effect_slayermappap",
        "effect_slayermappak",
        "effect_slayerdandyp",
        "effect_slayerdandyk",
        "effect_slayerpilebunker",
        "effect_slayerbumpahead",
        "effect_slayeritslate",
        "effect_slayerhammerair",
        "effect_slayerhammerland",
        "effect_slayerdandycross",
        "effect_attack11",
        "effect_attack12",
        "effect_attackdash",
        "effect_attacks3",
        "effect_slayer6htilt",
        "effect_escapeairslide",
        "effect_attackhi3",
        "effect_attacklw3",
        "effect_attacks4",
        "effect_attackhi4",
        "effect_attacklw4",
        "effect_attackairn",
        "effect_attackairf",
        "effect_attackairb",
        "effect_attackairhi",
        "effect_attackairlw",
        "effect_attacks3hi",
        "effect_attacks3lw",
        "effect_slayerfs",
        "effect_slayer2d",
        "effect_attacks4hi",
        "effect_attacks4lw",
    ] {
        agent.effect_acmd(script, empty, Priority::High);
    }
    agent.install();
}
